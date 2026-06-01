from __future__ import annotations

import traceback
from typing import Any

from range_translator_runtime.ai_log import TranslateAiLog
from range_translator_runtime.runtime_types import EventEmitter

from .contracts import (
    TranslationPromptProfile,
    TranslationRequest,
    TranslationResult,
)
from .prompting import build_output_schema, render_repair_prompt, render_translation_prompt
from .validation import extract_translation_batch


class TranslationOrchestrator:
    def __init__(self, prompt_repository: Any, ai_providers: dict[str, Any]) -> None:
        self.prompt_repository = prompt_repository
        self.ai_providers = ai_providers

    def translate(
        self,
        payload: dict[str, Any],
        emit_event: EventEmitter | None = None,
    ) -> dict[str, Any]:
        request = TranslationRequest.from_payload(payload)
        prompt = TranslationPromptProfile.from_payload(
            self.prompt_repository.load(request.prompt_profile_id)
        )
        provider = self.ai_providers.get(request.provider_id)
        if provider is None:
            raise RuntimeError(f"AI provider not found: {request.provider_id}")

        ai_log = TranslateAiLog(payload, prompt.as_log_payload())
        repair_count = 0

        try:
            if not request.items:
                result = TranslationResult(
                    provider_id=request.provider_id,
                    model=request.model,
                    prompt_profile=prompt.id,
                    detected_source=request.source_language,
                    items=tuple(),
                ).as_payload()
                ai_log.finalize_success(result, repair_count)
                return result

            model = provider.resolve_model(request.endpoint, request.model)
            output_schema = build_output_schema(request.items, prompt.output_schema_hint)
            rendered_prompt = render_translation_prompt(prompt, request, output_schema)
            request_payload = provider.build_chat_payload(
                model,
                prompt.system_prompt,
                rendered_prompt,
                temperature=prompt.temperature,
                top_p=prompt.top_p,
            )
            ai_log.add_chat_request(attempt=1, repair=False, payload=request_payload)
            content = provider.chat_json(request.endpoint, request_payload)
            ai_log.add_model_output(attempt=1, repair=False, content=content)

            try:
                detected_source, translated_items = extract_translation_batch(
                    content,
                    request.items,
                    request.source_language,
                    request.target_language,
                )
            except RuntimeError as error:
                repair_count = 1
                repair_prompt = render_repair_prompt(
                    prompt,
                    request,
                    output_schema,
                    str(error),
                )
                repair_request_payload = provider.build_chat_payload(
                    model,
                    prompt.system_prompt,
                    repair_prompt,
                    temperature=prompt.temperature,
                    top_p=prompt.top_p,
                )
                ai_log.add_chat_request(
                    attempt=2,
                    repair=True,
                    payload=repair_request_payload,
                )
                repaired_content = provider.chat_json(request.endpoint, repair_request_payload)
                ai_log.add_model_output(attempt=2, repair=True, content=repaired_content)
                detected_source, translated_items = extract_translation_batch(
                    repaired_content,
                    request.items,
                    request.source_language,
                    request.target_language,
                )

            if emit_event is not None:
                for index, item in enumerate(translated_items):
                    partial_payload = {
                        "sourceId": item.id,
                        "index": item.index,
                        "providerId": request.provider_id,
                        "model": model,
                        "promptProfile": prompt.id,
                        "detectedSource": detected_source,
                        "translatedText": item.translation,
                        "confidence": item.confidence,
                        "done": index == len(translated_items) - 1,
                    }
                    emit_event("translation_partial", partial_payload)
                    ai_log.add_partial_event(partial_payload)

            result = TranslationResult(
                provider_id=request.provider_id,
                model=model,
                prompt_profile=prompt.id,
                detected_source=detected_source,
                items=tuple(translated_items),
            ).as_payload()
            ai_log.finalize_success(result, repair_count)
            return result
        except Exception as error:
            ai_log.finalize_error(
                error_message=str(error),
                traceback_text=traceback.format_exc(),
                repair_count=repair_count,
            )
            raise