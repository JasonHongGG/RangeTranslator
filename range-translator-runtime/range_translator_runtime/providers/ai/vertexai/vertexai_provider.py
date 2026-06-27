from __future__ import annotations

from google.cloud import aiplatform

from ..ai_provider import AIProvider, GenerateRequest, GenerateResponse

class VertexAIProvider(AIProvider):
    def __init__(self, model: str | None = None, project_id: str | None = None, region: str | None = None) -> None:
        if not project_id or not region:
            raise ValueError("project_id and region must be provided for VertexAIProvider")
            
        self._model = model or "gemini-1.5-pro-preview-0409"
        self._project_id = project_id
        self._region = region
        
        aiplatform.init(project=self._project_id, location=self._region)

    @property
    def name(self) -> str:
        return "vertexai"

    def generate(self, request: GenerateRequest) -> GenerateResponse:
        from vertexai.generative_models import GenerativeModel, GenerationConfig
        
        model = GenerativeModel(
            self._model,
            system_instruction=[request.system_prompt] if request.system_prompt else None,
        )

        config_args = {}
        if request.temperature is not None:
            config_args["temperature"] = request.temperature
        if request.max_tokens is not None:
            config_args["max_output_tokens"] = request.max_tokens
            
        generation_config = GenerationConfig(**config_args) if config_args else None
        contents = [request.prompt]

        response = model.generate_content(
            contents=contents,
            generation_config=generation_config,
        )

        text = response.text if response.text else ""
        
        usage = {
            "promptTokens": response.usage_metadata.prompt_token_count if response.usage_metadata else 0,
            "completionTokens": response.usage_metadata.candidates_token_count if response.usage_metadata else 0,
            "totalTokens": response.usage_metadata.total_token_count if response.usage_metadata else 0,
        }

        return GenerateResponse(
            text=text,
            usage=usage,
            metadata={"provider": self.name, "model": self._model, "project_id": self._project_id, "region": self._region}
        )
