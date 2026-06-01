use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

const RUNTIME_DIR_NAME: &str = "range-translator-runtime";
const PYTHON_ENV_VAR: &str = "RANGE_TRANSLATOR_PYTHON";
const RUNTIME_ENV_VAR: &str = "RANGE_TRANSLATOR_RUNTIME_DIR";

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[derive(Debug, Clone)]
pub struct ResolvedCommand {
    pub program: String,
    pub args: Vec<String>,
}

pub fn find_runtime_root() -> Result<PathBuf, String> {
    for root in candidate_runtime_roots() {
        if root.join("pyproject.toml").exists() && root.join("range_translator_runtime").exists() {
            cleanup_unused_debug_runtime_copy(&root);
            return Ok(root);
        }
    }

    Err(format!(
        "sidecar runtime project not found. Expected {RUNTIME_DIR_NAME}/ in the workspace, next to the bundled executable, or via {RUNTIME_ENV_VAR}."
    ))
}

pub fn find_python_runtime(runtime_root: &PathBuf) -> Result<ResolvedCommand, String> {
    if let Ok(custom_python) = std::env::var(PYTHON_ENV_VAR) {
        if !custom_python.trim().is_empty() && can_execute(&custom_python, &[]) {
            return Ok(ResolvedCommand {
                program: custom_python,
                args: Vec::new(),
            });
        }
    }

    let windows_candidate = runtime_root
        .join(".venv")
        .join("Scripts")
        .join("python.exe");
    if windows_candidate.exists() {
        return Ok(ResolvedCommand {
            program: windows_candidate.to_string_lossy().to_string(),
            args: Vec::new(),
        });
    }

    let unix_candidate = runtime_root.join(".venv").join("bin").join("python");
    if unix_candidate.exists() {
        return Ok(ResolvedCommand {
            program: unix_candidate.to_string_lossy().to_string(),
            args: Vec::new(),
        });
    }

    let candidates = [
        ResolvedCommand {
            program: "py".to_string(),
            args: vec!["-3.12".to_string()],
        },
        ResolvedCommand {
            program: "py".to_string(),
            args: vec!["-3.11".to_string()],
        },
        ResolvedCommand {
            program: "python".to_string(),
            args: Vec::new(),
        },
        ResolvedCommand {
            program: "python3".to_string(),
            args: Vec::new(),
        },
        ResolvedCommand {
            program: "py".to_string(),
            args: vec!["-3".to_string()],
        },
    ];

    for candidate in candidates {
        if can_execute(&candidate.program, &candidate.args) {
            return Ok(candidate);
        }
    }

    Err(format!(
        "Python runtime not found. Create {RUNTIME_DIR_NAME}/.venv, set {PYTHON_ENV_VAR}, or point {RUNTIME_ENV_VAR} at a prepared runtime project."
    ))
}

fn candidate_runtime_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Ok(custom_root) = std::env::var(RUNTIME_ENV_VAR) {
        let custom_root = custom_root.trim();
        if !custom_root.is_empty() {
            push_unique(&mut roots, PathBuf::from(custom_root));
        }
    }

    if let Ok(current_dir) = std::env::current_dir() {
        if current_dir
            .file_name()
            .map(|name| name == RUNTIME_DIR_NAME)
            .unwrap_or(false)
        {
            push_unique(&mut roots, current_dir.clone());
        }

        push_unique(&mut roots, current_dir.join(RUNTIME_DIR_NAME));
    }

    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let manifest_dir = PathBuf::from(manifest_dir);
        if let Some(workspace_root) = manifest_dir.parent() {
            push_unique(&mut roots, workspace_root.join(RUNTIME_DIR_NAME));
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            push_unique(&mut roots, dir.join(RUNTIME_DIR_NAME));
            if let Some(parent) = dir.parent() {
                push_unique(&mut roots, parent.join(RUNTIME_DIR_NAME));
            }
        }
    }

    roots
}

fn push_unique(roots: &mut Vec<PathBuf>, candidate: PathBuf) {
    if roots.iter().any(|existing| existing == &candidate) {
        return;
    }
    roots.push(candidate);
}

fn cleanup_unused_debug_runtime_copy(selected_root: &Path) {
    let Ok(exe_path) = std::env::current_exe() else {
        return;
    };

    let Some(exe_dir) = exe_path.parent() else {
        return;
    };

    let debug_runtime_root = exe_dir.join(RUNTIME_DIR_NAME);
    if selected_root == debug_runtime_root {
        return;
    }

    if !debug_runtime_root.exists() {
        return;
    }

    let _ = fs::remove_dir_all(debug_runtime_root);
}

fn can_execute(program: &str, args: &[String]) -> bool {
    let mut command = hidden_command(program);
    for arg in args {
        command.arg(arg);
    }

    command
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

#[cfg(target_os = "windows")]
pub fn hidden_command(program: &str) -> Command {
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let mut command = Command::new(program);
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

#[cfg(not(target_os = "windows"))]
pub fn hidden_command(program: &str) -> Command {
    Command::new(program)
}
