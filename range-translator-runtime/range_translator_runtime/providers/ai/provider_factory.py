from __future__ import annotations

from range_translator_runtime.core import Config
from .ai_provider import AIProvider

class ProviderFactory:
    @staticmethod
    def create_provider(agent_name: str) -> AIProvider:
        if agent_name.lower() == "translator":
            provider_type = Config.get_translator_provider()
            model = Config.get_translator_model()
        else:
            raise RuntimeError(f"Configuration Error: Unknown agent type '{agent_name}'")

        if provider_type.lower() == "ollama":
            from .ollama.ollama_provider import OllamaProvider
            return OllamaProvider(model=model)
            
        elif provider_type.lower() == "vertex":
            from .vertexai.vertexai_provider import VertexAIProvider
            project_id = Config.get_vertex_project_id()
            region = Config.get_vertex_region()
            return VertexAIProvider(model=model, project_id=project_id, region=region)
            
        elif provider_type.lower() == "geminiflow":
            from .geminiflow.geminiflow_provider import GeminiFlowProvider
            return GeminiFlowProvider(model=model)
            
        else:
            raise RuntimeError(f"Configuration Error: Unsupported provider type '{provider_type}' for agent '{agent_name}'")
