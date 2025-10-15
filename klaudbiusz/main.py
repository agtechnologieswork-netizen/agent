#!/usr/bin/env python3
"""Simplified Claude Code CLI that spawns dabgent-mcp server."""

import asyncio
import os
import sys
from datetime import datetime, timezone
from pathlib import Path
from uuid import UUID, uuid4
import fire
import coloredlogs
from dotenv import load_dotenv
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

try:
    import asyncpg  # type: ignore[import-untyped]
except ImportError:
    asyncpg = None


class TrackerDB:
    """Simple Neon/Postgres tracker for message logging."""

    def __init__(self, wipe_on_start: bool = True):
        load_dotenv()
        self.database_url = os.getenv("DATABASE_URL")
        self.wipe_on_start = wipe_on_start
        self.pool = None

    async def init(self) -> None:
        """Initialize DB connection and schema."""
        if not self.database_url or not asyncpg:
            return

        try:
            self.pool = await asyncpg.create_pool(self.database_url, min_size=1, max_size=5)

            async with self.pool.acquire() as conn:
                if self.wipe_on_start:
                    await conn.execute("DROP TABLE IF EXISTS messages")

                await conn.execute("""
                    CREATE TABLE IF NOT EXISTS messages (
                        id UUID PRIMARY KEY,
                        role TEXT NOT NULL,
                        message_type TEXT NOT NULL,
                        message TEXT NOT NULL,
                        datetime TIMESTAMP NOT NULL,
                        run_id UUID NOT NULL
                    )
                """)
        except Exception as e:
            print(f"⚠️  DB init failed: {e}", file=sys.stderr)
            self.pool = None

    async def log(self, run_id: UUID, role: str, message_type: str, message: str) -> None:
        """Log a message to the database."""
        if not self.pool:
            return

        try:
            async with self.pool.acquire() as conn:
                await conn.execute(
                    "INSERT INTO messages (id, role, message_type, message, datetime, run_id) VALUES ($1, $2, $3, $4, $5, $6)",
                    uuid4(),
                    role,
                    message_type,
                    message,
                    datetime.now(timezone.utc).replace(tzinfo=None),
                    run_id,
                )
        except Exception as e:
            print(f"⚠️  DB log failed: {e}", file=sys.stderr)

    async def close(self) -> None:
        """Close DB connection pool."""
        if self.pool:
            await self.pool.close()


class SimplifiedClaudeCode:
    """CLI for running Claude Code with dabgent-mcp integration."""

    def __init__(self, wipe_db: bool = True):
        """Initialize the CLI."""
        # configure colored logging
        coloredlogs.install(level="INFO")

        # determine path to dabgent-mcp manifest
        self.project_root = Path(__file__).parent.parent
        self.mcp_manifest = self.project_root / "dabgent" / "dabgent_mcp" / "Cargo.toml"

        if not self.mcp_manifest.exists():
            raise RuntimeError(f"dabgent-mcp Cargo.toml not found at {self.mcp_manifest}")

        # tracker for DB logging
        self.tracker = TrackerDB(wipe_on_start=wipe_db)
        self.run_id: UUID = uuid4()  # will be reset at run_async start

    async def run_async(self, prompt: str) -> None:
        """Run the agent with the given prompt.

        Args:
            prompt: User prompt for the agent
        """
        # init tracker and generate run ID
        await self.tracker.init()
        self.run_id = uuid4()
        await self.tracker.log(self.run_id, "user", "prompt", f"run_id: {self.run_id}, prompt: {prompt}")

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
                await self._log_message(message)
        except Exception as e:
            print(f"\n❌ Error: {e}", file=sys.stderr)
            raise
        finally:
            await self.tracker.close()

    async def _log_message(self, message) -> None:
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
                    await self.tracker.log(self.run_id, "assistant", "text", block.text)
                elif isinstance(block, ToolUseBlock):
                    # format tool parameters nicely
                    params = ", ".join(f"{k}={v}" for k, v in (block.input or {}).items())
                    logger.info(f"🔧 Tool: {block.name}({truncate(params, 150)})")
                    await self.tracker.log(self.run_id, "assistant", "tool_call", f"{block.name}({params})")

        elif isinstance(message, UserMessage):
            for block in message.content:
                if isinstance(block, ToolResultBlock):
                    if block.is_error:
                        logger.warning(f"❌ Tool error: {truncate(str(block.content))}")
                        await self.tracker.log(self.run_id, "user", "tool_error", str(block.content))
                    else:
                        result_text = str(block.content)
                        if result_text:
                            logger.info(f"✅ Tool result: {truncate(result_text)}")
                            await self.tracker.log(self.run_id, "user", "tool_result", result_text)

        elif isinstance(message, ResultMessage):
            usage = message.usage or {}
            input_tokens = usage.get("input_tokens", 0)
            output_tokens = usage.get("output_tokens", 0)
            cache_creation = usage.get("cache_creation_input_tokens", 0)
            cache_read = usage.get("cache_read_input_tokens", 0)

            logger.info(f"🏁 Session complete: {message.num_turns} turns, ${message.total_cost_usd:.4f}")
            logger.info(f"   Tokens - in: {input_tokens}, out: {output_tokens}, cache_create: {cache_creation}, cache_read: {cache_read}")
            if message.result:
                logger.info(f"Final result: {truncate(message.result)}")

            await self.tracker.log(
                self.run_id,
                "result",
                "complete",
                f"turns={message.num_turns}, cost=${message.total_cost_usd:.4f}, tokens_in={input_tokens}, tokens_out={output_tokens}, cache_create={cache_creation}, cache_read={cache_read}, result={message.result or 'N/A'}"
            )

    def run(self, prompt: str, wipe_db: bool = True) -> None:
        """CLI entry point.

        Args:
            prompt: User prompt for the agent
            wipe_db: Whether to wipe the DB on start (default: True)
        """
        self.tracker.wipe_on_start = wipe_db
        asyncio.run(self.run_async(prompt))


def main():
    """Fire CLI entry point."""
    cli = SimplifiedClaudeCode()
    fire.Fire(cli.run)


if __name__ == "__main__":
    main()
