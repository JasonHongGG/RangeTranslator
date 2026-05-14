from __future__ import annotations

import json
import sys
import traceback
from pathlib import Path
from typing import Any

from range_translator_runtime.application import build_default_application


def main() -> int:
    runtime_script = Path(__file__).resolve()
    subcommand = sys.argv[1] if len(sys.argv) > 1 else "status"

    if subcommand == "serve":
        return serve(runtime_script)

    application = build_default_application(runtime_script)
    payload = _read_payload()
    try:
        result = application.dispatch(subcommand, payload)
        sys.stdout.write(json.dumps(result))
        return 0
    except Exception as error:  # pragma: no cover - CLI error path
        sys.stderr.write(
            json.dumps(
                {
                    "error": str(error),
                    "traceback": traceback.format_exc(),
                }
            )
        )
        return 1


def serve(runtime_script: Path) -> int:
    application = build_default_application(runtime_script)

    while True:
        raw_line = sys.stdin.buffer.readline()
        if not raw_line:
            break

        line = raw_line.decode("utf-8").strip()
        if not line:
            continue

        request_id: Any = None
        response: dict[str, Any]

        try:
            request = json.loads(line)
            request_id = request.get("requestId")
            subcommand = request.get("subcommand", "status")
            payload = request.get("payload") or {}

            def emit_event(event_name: str, event_payload: dict[str, Any]) -> None:
                sys.stdout.write(
                    json.dumps(
                        {
                            "requestId": request_id,
                            "event": event_name,
                            "payload": event_payload,
                        }
                    )
                    + "\n"
                )
                sys.stdout.flush()

            result = application.dispatch(subcommand, payload, emit_event)
            response = {
                "requestId": request_id,
                "ok": True,
                "result": result,
            }
        except Exception as error:  # pragma: no cover - serve loop error path
            response = {
                "requestId": request_id,
                "ok": False,
                "error": str(error),
                "traceback": traceback.format_exc(),
            }

        sys.stdout.write(json.dumps(response) + "\n")
        sys.stdout.flush()

    return 0


def _read_payload() -> dict[str, Any]:
    if sys.stdin.isatty():
        return {}

    raw = sys.stdin.buffer.read().decode("utf-8").strip()
    if not raw:
        return {}

    return json.loads(raw)