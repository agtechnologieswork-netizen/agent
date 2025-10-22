#!/usr/bin/env python3
"""
Agentic evaluation script - uses Claude with bash tools to evaluate apps.

Instead of hardcoded logic, gives Claude a generic prompt and bash tools
to discover how to build, run, test, and evaluate each app.
"""
from __future__ import annotations

import asyncio
import json
from pathlib import Path

from claude_agent_sdk import ClaudeAgentOptions, query


EVAL_PROMPT = """Evaluate all apps in ../app using the evaluation framework in ../eval-docs/evals.md.

For each app:
1. Read its files to understand what it is
2. Try to build and run it
3. Generate a report

Save results to evaluation_report.json and EVALUATION_REPORT.md in the project root.
"""


async def main():
    """Run agentic evaluation."""
    print("🤖 Starting evaluation...")

    options = ClaudeAgentOptions(
        permission_mode="bypassPermissions",
        max_turns=100,
    )

    async for message in query(prompt=EVAL_PROMPT, options=options):
        pass

    print("✅ Done! Check evaluation_report.json and EVALUATION_REPORT.md")


if __name__ == "__main__":
    asyncio.run(main())
