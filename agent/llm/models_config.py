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
- devstral (Mistral 24B)
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
LLM_FAST_MODEL=phi4               # Phi4 14B - good model from Microsoft
LLM_CODEGEN_MODEL=devstral        # Mistral 24B - excellent for code
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


class ModelCategory:
    FAST = "fast"
    CODEGEN = "codegen"
    VISION = "vision"

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
    "devstral": {"ollama": "devstral:24b"},     # Latest code-specialized
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
    ModelCategory.FAST: "gemini-flash-lite",
    ModelCategory.CODEGEN: "sonnet", 
    ModelCategory.VISION: "gemini-flash-lite",
}

OLLAMA_DEFAULT_MODELS = {
    ModelCategory.FAST: "phi4",
    ModelCategory.CODEGEN: "devstral",
    ModelCategory.VISION: "gemma3",
}

def get_model_for_category(category: str) -> str:
    """Get model name for a specific category, with environment variable override support."""
    env_var = f"LLM_{category.upper()}_MODEL"
    
    # Check for explicit model override first
    if explicit_model := os.getenv(env_var):
        return explicit_model
    
    # If PREFER_OLLAMA is set, use Ollama models as default
    if os.getenv("PREFER_OLLAMA"):
        return OLLAMA_DEFAULT_MODELS.get(category, "gemma3")
    
    # Otherwise use regular defaults
    return DEFAULT_MODELS.get(category, "sonnet")

ANTHROPIC_MODEL_NAMES = list(ANTHROPIC_MODELS.keys())
GEMINI_MODEL_NAMES = list(GEMINI_MODELS.keys())
OLLAMA_MODEL_NAMES = list(OLLAMA_MODELS.keys())

ALL_MODEL_NAMES = ANTHROPIC_MODEL_NAMES + GEMINI_MODEL_NAMES + OLLAMA_MODEL_NAMES
