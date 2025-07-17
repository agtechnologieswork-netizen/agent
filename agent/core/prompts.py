"""
Unified prompt template system for all agents.

Simple, direct approach - no over-engineered registry pattern.
"""

from enum import Enum
from typing import List, Optional

# Import all rule modules at top level
from core.prompt_rules.common import get_common_rules
from core.prompt_rules.python import get_python_rules
from core.prompt_rules.typescript import get_typescript_rules
from core.prompt_rules.integrations.databricks import get_databricks_rules


class PromptKind(Enum):
    """Prompt type - just user or system."""
    SYSTEM = "system"
    USER = "user"


class Stage(Enum):
    """Development stage - separate from prompt kind."""
    DRAFT = "draft"
    HANDLER = "handler"
    FRONTEND = "frontend"
    BACKEND = "backend"
    EDIT = "edit"
    VALIDATION = "validation"


class Framework(Enum):
    """Framework enum that implies the language."""
    NICEGUI = "nicegui"  # Python
    TRPC = "trpc"       # TypeScript
    
    @property
    def language(self) -> str:
        """Get the language for this framework."""
        return {
            Framework.NICEGUI: "python",
            Framework.TRPC: "typescript"
        }[self]


# Simple prompt storage - just functions, no classes needed
_PROMPTS = {}


def register_prompt(
    framework: Framework,
    kind: PromptKind,
    content: str,
    integrations: Optional[List[str]] = None
) -> None:
    """Register a prompt template."""
    key = _make_key(framework, kind, integrations or [])
    _PROMPTS[key] = content


def get_prompt_template(
    framework: Framework,
    kind: PromptKind,
    integrations: Optional[List[str]] = None
) -> str:
    """Get a prompt template - raises if not found."""
    key = _make_key(framework, kind, integrations or [])
    if key not in _PROMPTS:
        raise ValueError(f"No prompt found for {framework.value}:{kind.value}")
    return _PROMPTS[key]


def build_prompt(
    framework: Framework,
    kind: PromptKind,
    integrations: Optional[List[str]] = None,
    **kwargs
) -> str:
    """Build a complete prompt by combining rules with framework-specific content."""
    integrations = integrations or []
    
    # Get the framework prompt template
    template_content = get_prompt_template(framework, kind, integrations)
    
    # Get language-specific rules
    lang_rules = ""
    if framework.language == "python":
        lang_rules = get_python_rules()
    elif framework.language == "typescript":
        lang_rules = get_typescript_rules()
    
    # Get integration-specific rules
    integration_rules = ""
    if "databricks" in integrations:
        integration_rules += get_databricks_rules()
    
    # Combine all rules
    common_rules = get_common_rules()
    combined_rules = f"{common_rules}\n\n{lang_rules}\n\n{integration_rules}".strip()
    
    # Build the final prompt
    final_prompt = f"{combined_rules}\n\n{template_content}"
    
    # Apply template variables if provided
    if kwargs:
        try:
            import jinja2
            template = jinja2.Template(final_prompt)
            final_prompt = template.render(**kwargs)
        except Exception as e:
            raise ValueError(f"Template rendering error: {e}")
    
    return final_prompt


def _make_key(framework: Framework, kind: PromptKind, integrations: List[str]) -> str:
    """Make a simple key for prompt lookup."""
    if integrations:
        integrations_str = ":" + ",".join(sorted(integrations))
    else:
        integrations_str = ""
    return f"{framework.value}:{kind.value}{integrations_str}"