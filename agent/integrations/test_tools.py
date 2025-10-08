#!/usr/bin/env python3
"""Test script to verify tool availability."""

import asyncio
import coloredlogs

from claude_agent_sdk import ClaudeAgentOptions, query, tool, create_sdk_mcp_server

coloredlogs.install(level="INFO")


async def test_tools():
    # create a simple test tool
    @tool(
        "test_tool",
        "A simple test tool",
        {"message": str}
    )
    async def test_tool_fn(args):
        return {
            "content": [
                {"type": "text", "text": f"Test tool received: {args['message']}"}
            ]
        }

    # create MCP server
    tools_server = create_sdk_mcp_server(
        name="test_server",
        version="1.0.0",
        tools=[test_tool_fn]
    )

    system_prompt = """You are a test assistant. Your task is to list all tools you have access to."""

    # configure agent options
    options = ClaudeAgentOptions(
        system_prompt=system_prompt,
        mcp_servers={"test_server": tools_server},
        allowed_tools=["test_tool"],
        max_turns=5,
    )

    prompt = "What tools do you have available? List them explicitly by name."

    print("Starting test...\n")
    async for message in query(prompt=prompt, options=options):
        print(f"Message: {message}\n")

    print("\nTest complete")


if __name__ == "__main__":
    asyncio.run(test_tools())
