"""
Model configuration for all LLM providers.
Centralizes model definitions with categorization for better maintainability.

Environment Variable Configuration:
==================================

You can override default models using environment variables:
- LLM_FAST_MODEL: For fast text tasks (commit messages, name generation)
- LLM_CODEGEN_MODEL: For code generation and reasoning
- LLM_VISION_MODEL: For vision and UI analysis tasks

Latest Model Recommendations (2025):
===================================

For FAST_TEXT tasks:
- ministral (Mistral 3B)
- phi4 (Microsoft 14B)
- gemini-flash-lite (Google)

For BEST_CODEGEN tasks:
- codestral (Mistral 22B)
- qwen3-coder (Qwen 7B)
- granite-code (IBM 20B)
- llama3.3 (Meta)

For VISION tasks:
- llama3.2-vision (Meta)
- gemini-flash (Google)
- qwen3 (Qwen 30B with vision)

Example .env configuration for best performance:
==============================================
# Recommended: local models
LLM_FAST_MODEL=ministral           # Mistral 3B - fastest for simple tasks
LLM_CODEGEN_MODEL=codestral        # Mistral 22B - excellent for code
LLM_VISION_MODEL=gemma3            # Google - best vision model

# Alternative 1 local models
# LLM_FAST_MODEL=phi4
# LLM_CODEGEN_MODEL=granite-code
# LLM_VISION_MODEL=llama3.2-vision

# Alternative 2 local models
# LLM_CODEGEN_MODEL=qwen3-coder     # Best for coding
# LLM_VISION_MODEL=qwen3            # Advanced vision with thinking mode

# Alternative: Cloud models (API keys required)
# LLM_FAST_MODEL=gemini-flash-lite
# LLM_CODEGEN_MODEL=sonnet
# LLM_VISION_MODEL=gemini-flash
"""

import os
from typing import Dict
from enum import Enum


class ModelCategory(Enum):
    """Enum for different LLM model categories/use cases"""
    FAST_TEXT = "fast"           # Fast models for simple text tasks (name generation, commit messages)
    BEST_CODEGEN = "codegen"     # Best models for code generation and reasoning
    VISION = "vision"            # Models optimized for vision and UI analysis tasks

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
    # Meta LLaMA
    "llama3.3": {"ollama": "llama3.3"},           # Latest LLaMA, best for general coding
    "llama3.2-vision": {"ollama": "llama3.2-vision"},  # Latest vision capabilities
    "gemma3": {"ollama": "gemma3:27b"},  # Latest vision capabilities
    "codellama": {"ollama": "codellama:13b"},     # Specialized for code
    
    # Qwen3
    "qwen3": {"ollama": "qwen3:30b"},             # Latest Qwen with thinking mode
    "qwen3-coder": {"ollama": "qwen3-coder:7b"}, # Specialized for coding
    
    # Mistral
    "mistral-large": {"ollama": "mistral-large:123b"}, # Latest large model
    "codestral": {"ollama": "codestral:22b"},     # Latest code-specialized
    "ministral": {"ollama": "ministral:3b"},      # Fast, small model
    
    # Microsoft Phi
    "phi4": {"ollama": "phi4:14b"},               # Latest Phi model
    
    # Specialized models
    "deepseek-coder": {"ollama": "deepseek-coder:33b"}, # Best for coding
    "granite-code": {"ollama": "granite-code:20b"},     # IBM's code model
}

MODELS_MAP: Dict[str, Dict[str, str]] = {
    **ANTHROPIC_MODELS,
    **GEMINI_MODELS,
    **OLLAMA_MODELS,
}

DEFAULT_MODELS = {
    ModelCategory.FAST_TEXT: "ministral",        # Fast Mistral model for quick tasks
    ModelCategory.BEST_CODEGEN: "codestral",     # Best code generation (non-Chinese)
    ModelCategory.VISION: "gemma3",     # Latest vision capabilities
}

def get_model_for_category(category: ModelCategory | str) -> str:
    """Get model name for a specific category, with environment variable override support."""
    if isinstance(category, ModelCategory):
        category_str = category.value
        enum_category = category
    else:
        category_str = category
        # Convert string to enum for DEFAULT_MODELS lookup
        enum_category = next((cat for cat in ModelCategory if cat.value == category), ModelCategory.BEST_CODEGEN)
    
    env_var = f"LLM_{category_str.upper()}_MODEL"
    return os.getenv(env_var, DEFAULT_MODELS.get(enum_category, "sonnet"))

ANTHROPIC_MODEL_NAMES = list(ANTHROPIC_MODELS.keys())
GEMINI_MODEL_NAMES = list(GEMINI_MODELS.keys())
OLLAMA_MODEL_NAMES = list(OLLAMA_MODELS.keys())

ALL_MODEL_NAMES = ANTHROPIC_MODEL_NAMES + GEMINI_MODEL_NAMES + OLLAMA_MODEL_NAMES
