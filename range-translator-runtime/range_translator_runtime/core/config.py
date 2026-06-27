import os
from typing import Any

def _require_env(name: str) -> str:
    val = os.environ.get(name)
    if val is None or val.strip() == "":
        raise RuntimeError(f"Configuration Error: 必須設定環境變數 '{name}'")
    return val.strip()

def _require_int_env(name: str) -> int:
    val = _require_env(name)
    try:
        return int(val)
    except ValueError:
        raise RuntimeError(f"Configuration Error: 環境變數 '{name}' 必須為整數")

class Config:
    @classmethod
    def get_translator_provider(cls) -> str:
        return _require_env("AGENT_TRANSLATOR_PROVIDER")

    @classmethod
    def get_translator_model(cls) -> str:
        return _require_env("AGENT_TRANSLATOR_MODEL")

    @classmethod
    def get_ollama_url(cls) -> str:
        return _require_env("PROVIDER_OLLAMA_URL")

    @classmethod
    def get_ollama_timeout(cls) -> int:
        return _require_int_env("RANGE_TRANSLATOR_OLLAMA_CHAT_TIMEOUT_SECONDS")

    @classmethod
    def get_ollama_keep_alive(cls) -> str:
        return _require_env("RANGE_TRANSLATOR_OLLAMA_KEEP_ALIVE")

    @classmethod
    def get_geminiflow_url(cls) -> str:
        return _require_env("PROVIDER_GEMINIFLOW_URL")

    @classmethod
    def get_vertex_project_id(cls) -> str:
        return _require_env("PROVIDER_VERTEX_PROJECT_ID")

    @classmethod
    def get_vertex_region(cls) -> str:
        return _require_env("PROVIDER_VERTEX_REGION")

    @classmethod
    def get_ai_log_dir(cls) -> str | None:
        # 這是系統目錄設定，若無設定則允許為 None
        val = os.environ.get("RANGE_TRANSLATOR_AI_LOG_DIR")
        return val.strip() if val else None
