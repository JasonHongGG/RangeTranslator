from __future__ import annotations

from .base_agent import BaseAgent
from range_translator_runtime.providers.ai import ProviderFactory
from .translator_agent.index import TranslatorAgent

class AgentFactory:
    @staticmethod
    def create_agent(agent_name: str, **kwargs) -> BaseAgent:
        provider = ProviderFactory.create_provider(agent_name)

        if agent_name.lower() == "translator":
            return TranslatorAgent(provider=provider)
        
        else:
            raise RuntimeError(f"Factory Error: Unknown agent type '{agent_name}'")
