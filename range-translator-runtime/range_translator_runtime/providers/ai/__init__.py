from .ollama import OllamaProvider


def build_ai_providers() -> dict[str, OllamaProvider]:
    provider = OllamaProvider()
    return {provider.id: provider}


__all__ = ["OllamaProvider", "build_ai_providers"]
