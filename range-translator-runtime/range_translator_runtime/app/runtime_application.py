from __future__ import annotations

import os
from pathlib import Path
from typing import Any

from range_translator_runtime.prompts import PromptRepository
from range_translator_runtime.providers import build_ai_providers, build_ocr_providers
from range_translator_runtime.runtime_types import EventEmitter


class RuntimeApplication:
    def __init__(self, runtime_script: Path) -> None:
        self.runtime_root = resolve_runtime_root(runtime_script)
        prompt_root = Path(
            os.environ.get(
                "RANGE_TRANSLATOR_PROMPT_DIR",
                str(self.runtime_root / "prompts"),
            )
        )
        self.prompt_repository = PromptRepository(prompt_root)
        self.ai_providers = build_ai_providers()
        self.ocr_providers = build_ocr_providers()

    def dispatch(
        self,
        subcommand: str,
        payload: dict[str, Any],
        emit_event: EventEmitter | None = None,
    ) -> dict[str, Any]:
        if subcommand == "status":
            return self.status()
        if subcommand == "recognize":
            return self.recognize(payload)
        if subcommand == "translate":
            return self.translate(payload, emit_event)
        raise RuntimeError(f"Unsupported subcommand: {subcommand}")

    def status(self) -> dict[str, Any]:
        ocr_providers = [provider.descriptor() for provider in self.ocr_providers.values()]
        ai_providers = [provider.descriptor() for provider in self.ai_providers.values()]
        prompt_profiles = self.prompt_repository.list_profiles()

        return {
            "ocrProviders": ocr_providers,
            "aiProviders": ai_providers,
            "promptProfiles": prompt_profiles,
            "defaultOcrProviderId": self._default_provider_id(ocr_providers),
            "defaultAiProviderId": self._default_provider_id(ai_providers),
            "defaultPromptProfileId": self._default_prompt_profile_id(prompt_profiles),
        }

    def translate(
        self,
        payload: dict[str, Any],
        emit_event: EventEmitter | None = None,
    ) -> dict[str, Any]:
        provider_id = str(payload.get("providerId") or "ollama")
        prompt_profile = str(
            payload.get("promptProfile") or "translation.ui_overlay.default"
        )
        prompt = self.prompt_repository.load(prompt_profile)
        provider = self.ai_providers.get(provider_id)
        if provider is None:
            raise RuntimeError(f"AI provider not found: {provider_id}")
        return provider.translate(payload, prompt, emit_event)

    def recognize(self, payload: dict[str, Any]) -> dict[str, Any]:
        provider_id = str(
            payload.get("providerId") or self._default_provider_id_from_map(self.ocr_providers)
        )
        provider = self.ocr_providers.get(provider_id)
        if provider is None:
            raise RuntimeError(f"OCR provider not found: {provider_id}")
        return provider.recognize(payload)

    def _default_provider_id(self, providers: list[dict[str, Any]]) -> str | None:
        for provider in providers:
            if provider.get("available"):
                return str(provider.get("id"))
        return None

    def _default_provider_id_from_map(self, providers: dict[str, Any]) -> str:
        descriptors = [provider.descriptor() for provider in providers.values()]
        provider_id = self._default_provider_id(descriptors)
        if provider_id is None:
            details = [
                f"{descriptor.get('id')}: {descriptor.get('detail')}"
                for descriptor in descriptors
                if descriptor.get("detail")
            ]
            if details:
                raise RuntimeError(
                    "No available provider is registered. " + " | ".join(details)
                )
            raise RuntimeError("No available provider is registered")
        return provider_id

    def _default_prompt_profile_id(self, prompt_profiles: list[dict[str, Any]]) -> str | None:
        if not prompt_profiles:
            return None
        return str(prompt_profiles[0].get("id"))


def build_default_application(runtime_script: Path) -> RuntimeApplication:
    return RuntimeApplication(runtime_script)


def resolve_runtime_root(runtime_script: Path) -> Path:
    return runtime_script.parents[2]
