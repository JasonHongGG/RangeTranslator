from __future__ import annotations

from range_translator_runtime.agents import AgentFactory, BaseAgent
from range_translator_runtime.providers import build_ocr_providers

class AppContext:
    def __init__(self) -> None:
        self.translator_agent: BaseAgent = AgentFactory.create_agent("TRANSLATOR")
        self.ocr_providers = build_ocr_providers()

    def get_status(self) -> dict[str, Any]:
        ocr_providers_info = [provider.descriptor() for provider in self.ocr_providers.values()]
        ai_providers_info = [self.translator_agent.provider.descriptor()]

        default_ocr = self._default_provider_id(ocr_providers_info)
        
        return {
            "ocrProviders": ocr_providers_info,
            "aiProviders": ai_providers_info,
            "defaultOcrProviderId": default_ocr,
            "defaultAiProviderId": self.translator_agent.provider.name,
        }

    def _default_provider_id(self, providers: list[dict[str, Any]]) -> str | None:
        for provider in providers:
            if provider.get("available"):
                return str(provider.get("id"))
        return None

    def get_ocr_provider(self, provider_id: str | None) -> Any:
        if not provider_id:
            descriptors = [p.descriptor() for p in self.ocr_providers.values()]
            provider_id = self._default_provider_id(descriptors)

        if not provider_id or provider_id not in self.ocr_providers:
            raise RuntimeError(f"OCR provider not found: {provider_id}")
            
        return self.ocr_providers[provider_id]
