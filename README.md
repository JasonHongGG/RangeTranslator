# RangeTranslator

RangeTranslator is a desktop application that allows you to select a region on your screen, run Optical Character Recognition (OCR) to extract text, translate it, and render the translated text directly back over the original text using a transparent overlay.

## Features

- **Screen Region Selection**: Select any part of your screen to translate.
- **In-place Translation Overlay**: The translated text replaces the original text seamlessly on a transparent overlay.
- **AI-Powered Translation**: Leverages AI models via Ollama to provide natural, context-aware translations that read smoothly (not just literal word-for-word translations).
- **Allow Screenshot Mode**: Temporarily bypasses screenshot protection, allowing you to capture the translation overlay with system tools.

## Keyboard Shortcuts (Hotkeys)

The application supports the following hotkeys (make sure the main panel is focused):

- **`Ctrl + Shift + S`** (or `Cmd + Shift + S`): Open the screen selector to select a region.
- **`Ctrl + Enter`** (or `Cmd + Enter`): Start or Stop the real-time translation pipeline.
- **`Ctrl + Backspace`** (or `Cmd + Backspace`): Clear the current selection.
- **`Ctrl + Shift + D`** (or `Cmd + Shift + D`): Toggle "Allow Screenshot" mode (toggles window capture protection).
- **`Esc`**: Cancel and close the screen selector (when the selector is open).

## Setup & Requirements

1. **Ollama**: Ensure Ollama is running (locally or remotely). This app relies on Ollama for translation. Models designed for quick inference like `qwen3` are recommended.
2. **GPU (Recommended)**: For the best real-time OCR performance, a dedicated GPU is highly recommended. The backend utilizes PaddleOCR with GPU support (`paddlepaddle-gpu` cu129).

## Known Issues & Troubleshooting

Here are some special cases and known behaviors to be aware of:

- **Screenshot Tool Blocking (Click-through Issue)**:
  By default, the application protects its windows from being captured by screenshot tools to prevent an infinite loop where the OCR reads its own translated text. Enabling "Allow Screenshot" mode removes this protection. On Windows, changing this setting dynamically could cause the invisible overlay to lose its "click-through" property, mistakenly blocking mouse clicks on underlying apps (like the Snipping Tool or File Explorer). *This issue has been patched*, but if you experience any lingering click-blocking, toggling the "Allow Screenshot" setting again or clearing the selection will reset the window state.
- **Remote Ollama Timeout Issues**:
  If you are connecting to a remote Ollama instance (e.g., via ngrok), network latency or tunnel restrictions might cause the translation to hang. If translation stays stuck but the connection tests pass, check the remote server's response time or switch to a local Ollama instance.
- **AI "Thinking" Overhead**:
  When using reasoning models (like `qwen3`), the app explicitly disables the AI's internal "thinking" steps (`think: false`). This is intentional, as generating thought processes significantly delays real-time UI overlay translation (from ~1s to ~9s) without providing noticeable quality improvements for short text and UI labels.
- **GPU Compatibility**:
  If the application fails to run OCR, it might be due to missing CUDA libraries or unsupported PaddleOCR wheels. Ensure you have the correct CUDA 12.x dependencies for your specific GPU architecture (e.g., RTX 50-series requires the latest `cu129` wheels).
