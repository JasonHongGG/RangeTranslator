from __future__ import annotations

import traceback
from typing import Any

from range_translator_runtime.providers.logging import AILoggerProvider
from range_translator_runtime.providers.ai import AIProvider, GenerateRequest
from range_translator_runtime.core import EventEmitter, JsonMap
from ..base_agent import BaseAgent

from .contracts import (
    TranslationRequest,
    TranslationResult,
)
from .prompts import build_output_schema, build_system_prompt, build_user_prompt
from .validation import extract_translation_batch

class TranslatorAgent(BaseAgent):
    def __init__(self, provider: AIProvider) -> None:
        super().__init__("TranslatorAgent", provider)

    def execute(self, payload: JsonMap, emit_event: EventEmitter | None = None) -> JsonMap:
        self.logger.info("Executing translation request")
        request = TranslationRequest.from_payload(payload)
        provider = self.provider

        prompt_metadata = {
            "role": "Desktop UI Translator",
            "goal": "Translate OCR text captured from a live desktop overlay into polished, natural UI wording."
        }
        
        ai_log = AILoggerProvider("translate", payload, prompt_metadata)
        repair_count = 0

        try:
            if not request.items:
                result = TranslationResult(
                    provider_id=provider.name,
                    detected_source=request.source_language,
                    items=tuple(),
                ).as_payload()
                ai_log.finalize_success(result)
                return result

            output_schema = build_output_schema(request.items)
            system_prompt = build_system_prompt()
            user_prompt = build_user_prompt(request, output_schema)
            
            gen_request = GenerateRequest(
                prompt=user_prompt,
                system_prompt=system_prompt,
                temperature=0.18,
            )
            
            ai_log.add_chat_request(attempt=1, payload=gen_request.__dict__)
            
            self.logger.info(f"Generating content on {provider.name}")
            response = provider.generate(gen_request)
            content = response.text
            ai_log.add_model_output(attempt=1, content=content)

            try:
                detected_source, translated_items = extract_translation_batch(
                    content,
                    request.items,
                    request.source_language,
                    request.target_language,
                )
            except RuntimeError as error:
                self.logger.warning(f"Validation failed, starting repair: {error}")
                repair_count = 1
                
                repair_prompt = build_user_prompt(request, output_schema, str(error))
                repair_gen_request = GenerateRequest(
                    prompt=repair_prompt,
                    system_prompt=system_prompt,
                    temperature=0.18,
                )
                
                ai_log.add_chat_request(attempt=2, payload=repair_gen_request.__dict__)
                
                repaired_response = provider.generate(repair_gen_request)
                repaired_content = repaired_response.text
                ai_log.add_model_output(attempt=2, content=repaired_content)
                
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
                        "providerId": provider.name,
                        "detectedSource": detected_source,
                        "translatedText": item.translation,
                        "confidence": item.confidence,
                        "done": index == len(translated_items) - 1,
                    }
                    emit_event("translation_partial", partial_payload)
                    ai_log.add_partial_event(partial_payload)

            result = TranslationResult(
                provider_id=provider.name,
                detected_source=detected_source,
                items=tuple(translated_items),
            ).as_payload()
            
            ai_log.set_custom_metadata("repairCount", repair_count)
            ai_log.finalize_success(result)
            return result
        except Exception as error:
            self.logger.error(f"Execution failed: {error}")
            ai_log.set_custom_metadata("repairCount", repair_count)
            ai_log.finalize_error(
                error_message=str(error),
                traceback_text=traceback.format_exc(),
            )
            raise