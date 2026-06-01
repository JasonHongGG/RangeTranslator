use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, ChildStdout, Stdio},
    sync::OnceLock,
};

use parking_lot::Mutex;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::protocol::{
    RuntimeFrame, RuntimeInvokeError, RuntimeRequest, decode_payload, format_worker_error,
};
use super::runtime_process::{find_python_runtime, find_runtime_root, hidden_command};

const RETRY_LIMIT: usize = 1;

static SIDECAR_TRANSPORT: OnceLock<Mutex<Option<PersistentRuntimeTransport>>> = OnceLock::new();

struct PersistentRuntimeTransport {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_request_id: u64,
}

impl PersistentRuntimeTransport {
    fn start() -> Result<Self, String> {
        let runtime_root = find_runtime_root()?;
        let python = find_python_runtime(&runtime_root)?;
        let mut command = hidden_command(&python.program);
        for arg in &python.args {
            command.arg(arg);
        }

        let mut child = command
            .current_dir(&runtime_root)
            .arg("-m")
            .arg("range_translator_runtime")
            .arg("serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("failed to start sidecar runtime: {error}"))?;

        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stderr);
                let mut line = Vec::new();

                loop {
                    line.clear();
                    match reader.read_until(b'\n', &mut line) {
                        Ok(0) => break,
                        Ok(_) => {
                            let message = String::from_utf8_lossy(&line);
                            let message = message.trim();
                            if !message.is_empty() {
                                eprintln!("[RangeTranslator:sidecar] {message}");
                            }
                        }
                        Err(error) => {
                            eprintln!(
                                "[RangeTranslator:sidecar] failed to read sidecar stderr: {error}"
                            );
                            break;
                        }
                    }
                }
            });
        }

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "failed to open sidecar stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "failed to open sidecar stdout".to_string())?;

        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_request_id: 1,
        })
    }

    fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    fn invoke_streaming<TRequest, TResponse, TEvent, F>(
        &mut self,
        subcommand: &str,
        payload: &TRequest,
        mut on_event: F,
    ) -> Result<TResponse, RuntimeInvokeError>
    where
        TRequest: Serialize,
        TResponse: DeserializeOwned,
        TEvent: DeserializeOwned,
        F: FnMut(TEvent),
    {
        let request_id = self.next_request_id;
        self.next_request_id += 1;

        let request = RuntimeRequest {
            request_id,
            subcommand,
            payload,
        };

        let serialized = serde_json::to_vec(&request).map_err(|error| {
            RuntimeInvokeError::Unrecoverable(format!(
                "failed to serialize sidecar request: {error}"
            ))
        })?;

        self.stdin.write_all(&serialized).map_err(|error| {
            RuntimeInvokeError::Recoverable(format!("failed to write sidecar request: {error}"))
        })?;
        self.stdin.write_all(b"\n").map_err(|error| {
            RuntimeInvokeError::Recoverable(format!("failed to finalize sidecar request: {error}"))
        })?;
        self.stdin.flush().map_err(|error| {
            RuntimeInvokeError::Recoverable(format!("failed to flush sidecar request: {error}"))
        })?;

        loop {
            let mut line = String::new();
            let bytes = self.stdout.read_line(&mut line).map_err(|error| {
                RuntimeInvokeError::Recoverable(format!("failed to read sidecar response: {error}"))
            })?;

            if bytes == 0 {
                return Err(RuntimeInvokeError::Recoverable(
                    "sidecar runtime exited unexpectedly".to_string(),
                ));
            }

            let frame: RuntimeFrame = serde_json::from_str(line.trim_end()).map_err(|error| {
                RuntimeInvokeError::Recoverable(format!(
                    "failed to parse sidecar response: {error}\n{}",
                    line.trim()
                ))
            })?;

            if frame.request_id != request_id {
                return Err(RuntimeInvokeError::Recoverable(format!(
                    "mismatched sidecar response: expected request {request_id}, got {}",
                    frame.request_id
                )));
            }

            if frame.event.is_some() {
                let payload = frame.payload.unwrap_or(Value::Null);
                let event = decode_payload::<TEvent>(payload, "event payload")?;
                on_event(event);
                continue;
            }

            if frame.ok.unwrap_or(false) {
                let result = frame.result.ok_or_else(|| {
                    RuntimeInvokeError::Unrecoverable(
                        "sidecar returned success without a payload".to_string(),
                    )
                })?;

                return decode_payload(result, "success payload");
            }

            return Err(RuntimeInvokeError::Unrecoverable(format_worker_error(
                frame,
            )));
        }
    }
}

pub(crate) fn invoke<TRequest, TResponse>(
    subcommand: &str,
    payload: &TRequest,
) -> Result<TResponse, String>
where
    TRequest: Serialize,
    TResponse: DeserializeOwned,
{
    invoke_streaming(subcommand, payload, |_value: Value| {})
}

pub(crate) fn invoke_streaming<TRequest, TResponse, TEvent, F>(
    subcommand: &str,
    payload: &TRequest,
    mut on_event: F,
) -> Result<TResponse, String>
where
    TRequest: Serialize,
    TResponse: DeserializeOwned,
    TEvent: DeserializeOwned,
    F: FnMut(TEvent),
{
    let slot = transport_slot();
    let mut transport_guard = slot.lock();
    let mut attempt = 0usize;

    loop {
        let needs_restart = transport_guard
            .as_mut()
            .map(|transport| !transport.is_alive())
            .unwrap_or(true);

        if needs_restart {
            *transport_guard = Some(PersistentRuntimeTransport::start()?);
        }

        let Some(transport) = transport_guard.as_mut() else {
            return Err("sidecar runtime unavailable".to_string());
        };

        match transport.invoke_streaming::<TRequest, TResponse, TEvent, _>(
            subcommand,
            payload,
            &mut on_event,
        ) {
            Ok(response) => return Ok(response),
            Err(RuntimeInvokeError::Unrecoverable(error)) => return Err(error),
            Err(RuntimeInvokeError::Recoverable(error)) => {
                *transport_guard = None;
                if attempt >= RETRY_LIMIT {
                    return Err(error);
                }
                attempt += 1;
            }
        }
    }
}

fn transport_slot() -> &'static Mutex<Option<PersistentRuntimeTransport>> {
    SIDECAR_TRANSPORT.get_or_init(|| Mutex::new(None))
}
