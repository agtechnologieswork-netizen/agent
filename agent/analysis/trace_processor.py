#!/usr/bin/env python3
"""Simple trace processor that extracts all messages recursively"""

import json
import sys
from pathlib import Path
from typing import Dict, List, Any, Optional
from datetime import datetime
import re
from fire import Fire


def detect_agent_type_from_tools(messages: List[Dict]) -> str:
    """Detect agent type by analyzing tool usage patterns"""
    # Count file types
    file_counts = {"php": 0, "py": 0, "ts": 0}
    
    # Scan ALL messages and count file types
    for msg in messages:
        content = msg.get("content", "")
        if isinstance(content, list):
            for item in content:
                if isinstance(item, dict) and item.get("type") == "tool_use":
                    if item.get("name") == "write_file":
                        file_path = item.get("input", {}).get("path", "")
                        if file_path.endswith(".php"):
                            file_counts["php"] += 1
                        elif file_path.endswith(".py"):
                            file_counts["py"] += 1
                        elif file_path.endswith((".ts", ".tsx")):
                            file_counts["ts"] += 1
        elif isinstance(content, str):
            # check for tool use in string content
            if "write_file" in content:
                if ".php" in content:
                    file_counts["php"] += 1
                elif ".py" in content:
                    file_counts["py"] += 1
                elif ".ts" in content or ".tsx" in content:
                    file_counts["ts"] += 1
    
    # Find dominant file type
    if sum(file_counts.values()) == 0:
        return "unknown"
    
    dominant_type = max(file_counts.keys(), key=lambda k: file_counts[k])
    dominant_count = file_counts[dominant_type]
    
    # Only classify if there's actually a dominant pattern
    if dominant_count == 0:
        return "unknown"
    
    # Map dominant type to agent
    if dominant_type == "php":
        return "laravel_agent"
    elif dominant_type == "py":
        return "nicegui_agent"
    elif dominant_type == "ts":
        return "trpc_agent"
    
    return "unknown"




def format_content(content):
    """Format message content for display"""
    if not content:
        return ""

    # handle list content
    if isinstance(content, list):
        parts = []
        for item in content:
            if isinstance(item, dict):
                if item.get("type") == "text":
                    parts.append(item.get("text", ""))
                elif item.get("type") == "tool_use":
                    tool_name = item.get("name", "unknown")
                    tool_input = item.get("input", {})
                    tool_part = f"🔧 **{tool_name}**"
                    if tool_input:
                        input_json = json.dumps(tool_input, indent=2)
                        tool_part += f"\n```json\n{input_json}\n```"
                    parts.append(tool_part)
                elif item.get("type") == "tool_result":
                    is_error = item.get("is_error", False)
                    status = "❌" if is_error else "✅"
                    result_content = item.get("content", "")
                    result_part = f"{status} **Tool Result**"
                    if result_content:
                        result_part += f"\n```\n{result_content}\n```"
                    parts.append(result_part)
        return "\n".join(parts).strip()

    return str(content)


def extract_messages_recursive(obj: Any, messages: List[Dict] = None) -> List[Dict]:
    """Recursively extract all messages from any nested structure"""
    if messages is None:
        messages = []
    
    if isinstance(obj, dict):
        # if this dict has role and content, it's a message
        if "role" in obj and "content" in obj:
            messages.append(obj)
        # recurse into all values
        for value in obj.values():
            extract_messages_recursive(value, messages)
    elif isinstance(obj, list):
        # recurse into all items
        for item in obj:
            extract_messages_recursive(item, messages)
    
    return messages


def display_messages_to_file(messages: List[Dict], output_file):
    """Display messages chronologically to file"""
    print(f"📊 {len(messages)} messages extracted", file=output_file)
    print("=" * 80, file=output_file)
    
    for i, msg in enumerate(messages, 1):
        role = msg.get("role", "unknown")
        timestamp = msg.get("timestamp", "")
        
        # format role with emoji
        if role == "user":
            role_display = "👤 USER"
        elif role == "assistant":
            role_display = "🤖 ASSISTANT"
        else:
            role_display = f"📝 {role.upper()}"
        
        # format timestamp
        time_str = ""
        if timestamp:
            try:
                dt = datetime.fromisoformat(timestamp.replace("Z", "+00:00"))
                time_str = dt.strftime("%H:%M:%S")
            except (ValueError, TypeError):
                time_str = str(timestamp)[:20]  # truncate if invalid
        
        print(f"{i}. {role_display} {time_str}", file=output_file)
        print("-" * 40, file=output_file)
        
        content = format_content(msg.get("content", ""))
        print(content, file=output_file)
        print(file=output_file)


def process_trace_file(json_file: str, output: Optional[str] = None) -> str:
    """Process any trace file and extract all messages.
    
    Args:
        json_file: Path to the JSON trace file
        output: Optional output file path (if None, prints to stdout)
        
    Returns:
        Detected agent type
    """
    file_path = Path(json_file)

    if not file_path.exists():
        raise FileNotFoundError(f"File {file_path} not found")

    with open(file_path, "r") as f:
        data = json.load(f)

    # extract all messages recursively
    messages = extract_messages_recursive(data)
    
    if not messages:
        raise ValueError(f"No messages found in {file_path.name}")
    
    # detect agent type from tool usage
    agent_type = detect_agent_type_from_tools(messages)

    # determine output target
    if output:
        output_file = open(output, "w")
    else:
        output_file = sys.stdout

    try:
        print(f"Agent Type: {agent_type}", file=output_file)
        print(f"File: {file_path.name}", file=output_file)
        print("=" * 80, file=output_file)

        display_messages_to_file(messages, output_file)
    finally:
        if output:
            output_file.close()
            
    return agent_type


def main():
    """Main entry point for Fire CLI"""
    Fire(process_trace_file)


if __name__ == "__main__":
    main()