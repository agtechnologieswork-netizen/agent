# klaudbiusz

Wrapper for dabgent-mcp server with dual distribution methods.

## Overview

**klaudbiusz** provides two ways to use the dabgent-mcp server:

1. **CLI Wrapper** (`cli/main.py`) - For development and debugging
2. **Claude Code Plugin** (`.claude-plugin/`) - For distribution to users

Both invoke the same underlying `dabgent-mcp` Rust MCP server from `../dabgent/dabgent_mcp/`.


## CLI (main.py)

**Example:**
```bash
cd ~/dev/agent/klaudbiusz
export DATABASE_URL="postgresql://..."  # optional for logging
uv run cli/main.py "Create a simple dashboard"
```

## Plugin (.claude-plugin/)

**Use for:**
- Distributing to customers
- Claude Code integration
- End-user workflows
- PoC validation

**Installation:**
```
/plugin marketplace add ~/dev/agent/klaudbiusz
/plugin install klaudbiusz
```

Then restart Claude Code.
