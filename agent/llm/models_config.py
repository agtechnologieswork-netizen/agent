"""
Model configuration for all LLM providers.
Centralizes model definitions for better maintainability.
"""

from typing import Dict

ANTHROPIC_MODELS = {
    "sonnet": {
        "bedrock": "us.anthropic.claude-sonnet-4-20250514-v1:0",
        "anthropic": "claude-sonnet-4-20250514"
    },
    "haiku": {
        "bedrock": "us.anthropic.claude-3-5-haiku-20241022-v1:0",
        "anthropic": "claude-3-5-haiku-20241022"
    },
}

GEMINI_MODELS = {
    "gemini-pro": {
        "gemini": "gemini-2.5-pro-preview-05-06",
    },
    "gemini-flash": {
        "gemini": "gemini-2.5-flash-preview-05-20",
    },
    "gemini-flash-lite": {
        "gemini": "gemini-2.5-flash-lite-preview-06-17",
    },
}

OLLAMA_MODELS = {
    "llama3.2": {"ollama": "llama3.2"},
    "llama3.1": {"ollama": "llama3.1"},
    "codellama": {"ollama": "codellama"},
    "gemma3": {"ollama": "gemma3"},
    "deepseek-r1:32b": {"ollama": "deepseek-r1:32b"},
    "devstral:latest": {"ollama": "devstral:latest"},
    "qwen2.5vl:32b": {"ollama": "qwen2.5vl:32b"},
}

MODELS_MAP: Dict[str, Dict[str, str]] = {
    **ANTHROPIC_MODELS,
    **GEMINI_MODELS,
    **OLLAMA_MODELS,
}

ANTHROPIC_MODEL_NAMES = list(ANTHROPIC_MODELS.keys())
GEMINI_MODEL_NAMES = list(GEMINI_MODELS.keys())
OLLAMA_MODEL_NAMES = list(OLLAMA_MODELS.keys())

ALL_MODEL_NAMES = ANTHROPIC_MODEL_NAMES + GEMINI_MODEL_NAMES + OLLAMA_MODEL_NAMES
