#!/usr/bin/env python3
"""PowerApps code analyzer using Claude Agent SDK."""

import asyncio
import fire
import coloredlogs

from claude_agent_sdk import ClaudeAgentOptions, query, tool, create_sdk_mcp_server
from claude_agent_sdk.types import AssistantMessage, UserMessage, ToolUseBlock, TextBlock, ToolResultBlock, ResultMessage

from log import get_logger

coloredlogs.install(level="INFO")


logger = get_logger(__name__)


class PowerAppsAnalyzer:
    """Analyze unpacked PowerApps code and generate comprehensive spec."""

    def __init__(self):
        self.spec_result: str | None = None
        self.last_assistant_text: str | None = None

    def _log_message(self, message):

        if isinstance(message, AssistantMessage):
            for block in message.content:
                if isinstance(block, ToolUseBlock):
                    params = ', '.join(f'{k}={v}' for k, v in block.input.items())
                    logger.info(f"🔧 Tool: {block.name}({params})")
                elif isinstance(block, TextBlock):
                    # save for fallback if done() wasn't called
                    self.last_assistant_text = block.text
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

    def _create_done_tool(self):
        analyzer_ref = self

        @tool(
            "done",
            "Call this tool when analysis is complete. Pass the full technical specification as the spec parameter.",
            {"spec": str}
        )
        async def done_tool(args):
            analyzer_ref.spec_result = args["spec"]
            return {
                "content": [
                    {"type": "text", "text": "Specification received. Analysis complete."}
                ]
            }

        return done_tool

    async def analyze(self, app_dir: str) -> str:
        logger.info(f"Analyzing PowerApp at: {app_dir}")

        # create done tool and MCP server
        done_tool = self._create_done_tool()
        tools_server = create_sdk_mcp_server(
            name="powerapp-tools",
            version="1.0.0",
            tools=[done_tool]
        )

        system_prompt = """You are analyzing an unpacked Microsoft PowerApp.

Typical PowerApp structure:
- Src/ - Contains App.pa.yaml (main app), screen definitions (*.pa.yaml), and Components/ subfolder
- DataSources/ - Data source definitions and sample data
- Assets/ - Media files (images, videos, music)
- Connections/ - Connection references for external data sources

There may be additional folders and files, analyze them as needed.

Your task:
1. Explore the directory structure using Glob and Read tools
2. Analyze the code, formulas, and configurations
3. Generate a comprehensive technical specification covering:
   - App overview and core purpose
   - Screen hierarchy and navigation flow
   - Data model (collections, data sources, connections)
   - Component catalog and their usage
   - Key formulas and business logic patterns
   - UI controls and patterns
   - Notable features and functionality

CRITICAL: When you finish your analysis, call the 'done' tool with your complete specification.
The tool takes one parameter: spec (the markdown specification string).
DO NOT just output the specification as text - you must use the done tool.

Be thorough - read key files to understand the app deeply.
Efficiency tip: You can read up to 10 files in parallel - batch your Read tool calls for faster analysis."""

        # configure agent options
        options = ClaudeAgentOptions(
            system_prompt=system_prompt,
            mcp_servers={"powerapp_tools": tools_server},
            allowed_tools=["Read", "Grep", "Glob", "done"],
            max_turns=100,
        )

        prompt = f"Analyze the PowerApp located at {app_dir} and generate a comprehensive specification."

        logger.info("Starting agent analysis...")
        async for message in query(prompt=prompt, options=options):
            self._log_message(message)

        # prefer done() result, but fallback to last assistant text if agent didn't call it
        if self.spec_result is None:
            if self.last_assistant_text and len(self.last_assistant_text) > 500:
                logger.warning("Agent didn't call done(), using last assistant message as spec")
                self.spec_result = self.last_assistant_text
            else:
                raise RuntimeError("Agent did not call done() tool or provide specification text")

        logger.info("Analysis complete")
        return self.spec_result

    def run(self, app_dir: str) -> None:
        result = asyncio.run(self.analyze(app_dir))
        print(result)


def main():
    analyzer = PowerAppsAnalyzer()
    fire.Fire(analyzer.run)


if __name__ == "__main__":
    main()
