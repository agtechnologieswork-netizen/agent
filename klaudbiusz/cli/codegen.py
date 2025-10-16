import asyncio
import logging
import os
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import TypedDict
from uuid import UUID, uuid4

import coloredlogs
from claude_agent_sdk import (
    AssistantMessage,
    ClaudeAgentOptions,
    ResultMessage,
    TextBlock,
    ToolResultBlock,
    ToolUseBlock,
    UserMessage,
    query,
)
from dotenv import load_dotenv

try:
    import asyncpg  # type: ignore[import-untyped]
except ImportError:
    asyncpg = None

logger = logging.getLogger(__name__)


class GenerationMetrics(TypedDict):
    cost_usd: float
    input_tokens: int
    output_tokens: int
    turns: int


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
    def __init__(self, wipe_db: bool = True, suppress_logs: bool = False):
        self.project_root = Path(__file__).parent.parent
        self.mcp_manifest = self.project_root / "dabgent" / "dabgent_mcp" / "Cargo.toml"

        if not self.mcp_manifest.exists():
            raise RuntimeError(f"dabgent-mcp Cargo.toml not found at {self.mcp_manifest}")

        self.tracker = TrackerDB(wipe_on_start=wipe_db)
        self.run_id: UUID = uuid4()

        self.suppress_logs = suppress_logs
        if suppress_logs:
            logging.getLogger().setLevel(logging.ERROR)
        else:
            coloredlogs.install(level="INFO")

    async def run_async(self, prompt: str) -> GenerationMetrics:
        await self.tracker.init()
        self.run_id = uuid4()
        await self.tracker.log(self.run_id, "user", "prompt", f"run_id: {self.run_id}, prompt: {prompt}")

        options = ClaudeAgentOptions(
            system_prompt={
                "type": "preset",
                "preset": "claude_code",
                "append": """The project should start with initiate_project in ./app/ for scaffolding and validate_project is required to finish the work.\n
Make sure to add tests for what you're implementing.\n
Bias towards backend code when the task allows to implement it in multiple places.\n
Be concise and to the point in your responses.\n
Use up to 10 tools per call to speed up the process.\n""",
            },
            permission_mode="bypassPermissions",
            disallowed_tools=[
                "NotebookEdit",
                "WebSearch",
                "WebFetch",
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
                    "env": {},
                }
            },
        )

        if not self.suppress_logs:
            print(f"\n{'=' * 80}")
            print(f"Prompt: {prompt}")
            print(f"{'=' * 80}\n")

        metrics: GenerationMetrics = {
            "cost_usd": 0.0,
            "input_tokens": 0,
            "output_tokens": 0,
            "turns": 0,
        }

        try:
            async for message in query(prompt=prompt, options=options):
                await self._log_message(message)
                if isinstance(message, ResultMessage):
                    usage = message.usage or {}
                    metrics = {
                        "cost_usd": message.total_cost_usd,
                        "input_tokens": usage.get("input_tokens", 0),
                        "output_tokens": usage.get("output_tokens", 0),
                        "turns": message.num_turns,
                    }
        except Exception as e:
            if not self.suppress_logs:
                print(f"\n❌ Error: {e}", file=sys.stderr)
            raise
        finally:
            await self.tracker.close()

        return metrics

    async def _log_message(self, message) -> None:
        def truncate(text: str, max_len: int = 300) -> str:
            return text if len(text) <= max_len else text[:max_len] + "..."

        if isinstance(message, AssistantMessage):
            for block in message.content:
                if isinstance(block, TextBlock):
                    if not self.suppress_logs:
                        logger.info(f"💬 {block.text}")
                    await self.tracker.log(self.run_id, "assistant", "text", block.text)
                elif isinstance(block, ToolUseBlock):
                    params = ", ".join(f"{k}={v}" for k, v in (block.input or {}).items())
                    if not self.suppress_logs:
                        logger.info(f"🔧 Tool: {block.name}({truncate(params, 150)})")
                    await self.tracker.log(self.run_id, "assistant", "tool_call", f"{block.name}({params})")

        elif isinstance(message, UserMessage):
            for block in message.content:
                if isinstance(block, ToolResultBlock):
                    if block.is_error:
                        if not self.suppress_logs:
                            logger.warning(f"❌ Tool error: {truncate(str(block.content))}")
                        await self.tracker.log(self.run_id, "user", "tool_error", str(block.content))
                    else:
                        result_text = str(block.content)
                        if result_text:
                            if not self.suppress_logs:
                                logger.info(f"✅ Tool result: {truncate(result_text)}")
                            await self.tracker.log(self.run_id, "user", "tool_result", result_text)

        elif isinstance(message, ResultMessage):
            usage = message.usage or {}
            input_tokens = usage.get("input_tokens", 0)
            output_tokens = usage.get("output_tokens", 0)
            cache_creation = usage.get("cache_creation_input_tokens", 0)
            cache_read = usage.get("cache_read_input_tokens", 0)

            if not self.suppress_logs:
                logger.info(f"🏁 Session complete: {message.num_turns} turns, ${message.total_cost_usd:.4f}")
                logger.info(
                    f"   Tokens - in: {input_tokens}, out: {output_tokens}, cache_create: {cache_creation}, cache_read: {cache_read}"
                )
                if message.result:
                    logger.info(f"Final result: {truncate(message.result)}")

            await self.tracker.log(
                self.run_id,
                "result",
                "complete",
                f"turns={message.num_turns}, cost=${message.total_cost_usd:.4f}, tokens_in={input_tokens}, tokens_out={output_tokens}, cache_create={cache_creation}, cache_read={cache_read}, result={message.result or 'N/A'}",
            )

    def run(self, prompt: str, wipe_db: bool = True) -> GenerationMetrics:
        self.tracker.wipe_on_start = wipe_db
        return asyncio.run(self.run_async(prompt))
