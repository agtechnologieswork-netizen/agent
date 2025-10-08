#!/usr/bin/env python3
"""Seed data migration analyzer using Claude Agent SDK."""

import asyncio
from pathlib import Path
from typing import Any

import fire
import coloredlogs

from claude_agent_sdk import ClaudeAgentOptions, query
from claude_agent_sdk.types import AssistantMessage, UserMessage, ToolUseBlock, TextBlock, ToolResultBlock, ResultMessage

from log import get_logger

coloredlogs.install(level="INFO")

logger = get_logger(__name__)


class SeedMigrationAnalyzer:
    """Analyze seed data directory and generate migration script for target model."""

    def __init__(self) -> None:
        pass

    def _log_message(self, message: AssistantMessage | UserMessage | ResultMessage | Any) -> None:
        if isinstance(message, AssistantMessage):
            for block in message.content:
                if isinstance(block, ToolUseBlock):
                    params = ', '.join(f'{k}={v}' for k, v in block.input.items())
                    logger.info(f"🔧 Tool: {block.name}({params})")
                elif isinstance(block, TextBlock):
                    # truncate long text blocks
                    text_preview = block.text[:300] + "..." if len(block.text) > 300 else block.text
                    logger.info(f"💬 Assistant: {text_preview}")
                else:
                    logger.debug(f"📝 Assistant ({type(block).__name__}): {str(block)[:300]}")
        elif isinstance(message, UserMessage):
            for block in message.content:
                if isinstance(block, ToolResultBlock):
                    content_preview = str(block.content)[:300].replace('\n', ' ')
                    logger.info(f"✅ Result [{block.tool_use_id[-8:]}]: {content_preview}...")
                else:
                    logger.debug(f"👤 User ({type(block).__name__}): {str(block)[:300]}")
        elif isinstance(message, ResultMessage):
            logger.info(f"🏁 Session complete: {message.num_turns} turns, ${message.total_cost_usd:.4f}")
        else:
            logger.debug(f"📨 {type(message).__name__}: {str(message)[:300]}")

    def _load_target_model(self, app_dir: str) -> str:
        """Load target model definition from app_dir/server/src/db/schema.ts."""
        schema_path = Path(app_dir) / "server" / "src" / "db" / "schema.ts"
        if not schema_path.exists():
            raise FileNotFoundError(f"Schema file not found: {schema_path}")

        return schema_path.read_text()

    async def analyze(self, seed_dir: str, app_dir: str) -> None:
        logger.info(f"Analyzing seed data at: {seed_dir}")
        logger.info(f"App directory: {app_dir}")

        # load target model from hardcoded path
        schema_path = Path(app_dir) / "server" / "src" / "db" / "schema.ts"
        logger.info(f"Loading schema from: {schema_path}")
        model_definition = self._load_target_model(app_dir)

        # ensure scripts directory exists
        scripts_dir = Path(app_dir) / "server" / "src" / "scripts"
        scripts_dir.mkdir(parents=True, exist_ok=True)
        migration_script_path = scripts_dir / "migrate-seed-data.ts"

        system_prompt = f"""You are analyzing seed data to generate an automatic migration script.

Target Model Definition:
```
{model_definition}
```

Your task:
1. Explore the seed data directory structure using Glob and Read tools
2. Analyze existing seed data files (JSON, SQL, YAML, CSV, etc.)
3. Compare current data schema with the target model
4. Identify required transformations:
   - Field additions/deletions
   - Type conversions
   - Data normalization
   - Relationship changes
   - Default values for new fields
5. Generate a TypeScript Drizzle migration script that:
   - Uses APP_DATABASE_URL environment variable for database connection
   - Reads existing seed data
   - Transforms it to match target model
   - Handles edge cases and validation
   - Preserves data integrity
   - Can be executed as a standalone script
6. Write the migration script to {migration_script_path} using the Write tool

* NOTE: db.execute() requires sql`...` wrapper, not direct template literals.                                                                                                  ╎│
* Use: db.execute(sql`SELECT 1`)
* Not: db.execute`SELECT 1` ✗

CRITICAL: You MUST write the migration script to the exact path: {migration_script_path}
Do not use /tmp or any other directory. Use the Write tool with the full path provided above.
The script must use process.env.APP_DATABASE_URL for the database connection.

Be thorough - read all seed files to understand the current schema.
Efficiency tip: You can read up to 10 files in parallel - batch your Read tool calls for faster analysis."""

        # configure agent options
        options = ClaudeAgentOptions(
            system_prompt=system_prompt,
            allowed_tools=["Read", "Grep", "Glob", "Write"],
            max_turns=100,
        )

        prompt = f"Analyze the seed data at {seed_dir} and generate a migration script to transform it to the target schema. Write the script to {migration_script_path}."

        logger.info("Starting agent analysis...")
        async for message in query(prompt=prompt, options=options):
            self._log_message(message)

        logger.info("Analysis complete")

    def run(self, seed_dir: str, app_dir: str) -> None:
        """Run migration analysis and write script to app_dir/server/src/scripts/.

        Args:
            seed_dir: Path to directory containing seed data
            app_dir: Path to app directory (schema at app_dir/server/src/db/schema.ts)
        """
        asyncio.run(self.analyze(seed_dir, app_dir))


def main() -> None:
    analyzer = SeedMigrationAnalyzer()
    fire.Fire(analyzer.run)


if __name__ == "__main__":
    main()
