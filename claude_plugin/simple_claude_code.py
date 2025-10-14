#!/usr/bin/env python3
"""Simplified Claude Code CLI that spawns dabgent-mcp server."""

import asyncio
import sys
from pathlib import Path
import fire
import coloredlogs
from claude_agent_sdk import (
    query,
    ClaudeAgentOptions,
    AssistantMessage,
    UserMessage,
    ResultMessage,
    ToolUseBlock,
    ToolResultBlock,
    TextBlock,
)


class SimplifiedClaudeCode:
    """CLI for running Claude Code with dabgent-mcp integration."""

    def __init__(self):
        """Initialize the CLI."""
        # configure colored logging
        coloredlogs.install(level="INFO")

        # determine path to dabgent-mcp manifest
        self.project_root = Path(__file__).parent.parent
        self.mcp_manifest = self.project_root / "dabgent" / "dabgent_mcp" / "Cargo.toml"

        if not self.mcp_manifest.exists():
            raise RuntimeError(f"dabgent-mcp Cargo.toml not found at {self.mcp_manifest}")

    async def run_async(self, prompt: str) -> None:
        """Run the agent with the given prompt.

        Args:
            prompt: User prompt for the agent
        """
        # configure MCP server to spawn via cargo run
        options = ClaudeAgentOptions(
            system_prompt={
                "type": "preset",
                "preset": "claude_code",
                "append": "The project should start with initiate_project in ./app/ for scaffolding and validate_project is required to finish the work"
            },
            permission_mode="bypassPermissions",  # auto-accept all tool usage including MCP tools
            disallowed_tools=[
                "NotebookEdit",  # no jupyter support
                "WebSearch",     # no web search
                "WebFetch",      # no web fetching
            ],
            mcp_servers={
                "dabgent": {
                    "type": "stdio",
                    "command": "cargo",
                    "args": [
                        "run",
                        "--manifest-path",
                        str(self.mcp_manifest),
                    ],
                    "env": {
                        # suppress cargo output by not setting RUST_LOG
                    }
                }
            },
        )

        # stream responses
        print(f"\n{'='*80}")
        print(f"Prompt: {prompt}")
        print(f"{'='*80}\n")

        try:
            async for message in query(prompt=prompt, options=options):
                self._log_message(message)
        except Exception as e:
            print(f"\n❌ Error: {e}", file=sys.stderr)
            raise

    def _log_message(self, message) -> None:
        """Log a message with appropriate formatting.

        Args:
            message: Message to log
        """
        import logging
        logger = logging.getLogger(__name__)

        # truncate helper
        def truncate(text: str, max_len: int = 300) -> str:
            return text if len(text) <= max_len else text[:max_len] + "..."

        if isinstance(message, AssistantMessage):
            for block in message.content:
                if isinstance(block, TextBlock):
                    logger.info(f"💬 {block.text}")
                elif isinstance(block, ToolUseBlock):
                    # format tool parameters nicely
                    params = ", ".join(f"{k}={v}" for k, v in (block.input or {}).items())
                    logger.info(f"🔧 Tool: {block.name}({truncate(params, 150)})")

        elif isinstance(message, UserMessage):
            for block in message.content:
                if isinstance(block, ToolResultBlock):
                    if block.is_error:
                        logger.warning(f"❌ Tool error: {truncate(str(block.content))}")
                    else:
                        result_text = str(block.content)
                        if result_text:
                            logger.info(f"✅ Tool result: {truncate(result_text)}")

        elif isinstance(message, ResultMessage):
            logger.info(f"🏁 Session complete: {message.num_turns} turns, ${message.total_cost_usd:.4f}")
            if message.result:
                logger.info(f"Final result: {truncate(message.result)}")

    def run(self, prompt: str) -> None:
        """CLI entry point.

        Args:
            prompt: User prompt for the agent
        """
        asyncio.run(self.run_async(prompt))


def main():
    """Fire CLI entry point."""
    cli = SimplifiedClaudeCode()
    fire.Fire(cli.run)


if __name__ == "__main__":
    main()
