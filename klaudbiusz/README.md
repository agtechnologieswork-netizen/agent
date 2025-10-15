# klaudbiusz

Wrapper for dabgent-mcp server with dual distribution methods.

## Overview

**klaudbiusz** provides two ways to use the dabgent-mcp server:

1. **CLI Wrapper** (`main.py`) - For development and debugging
2. **Claude Code Plugin** (`.claude-plugin/`) - For distribution to users

Both invoke the same underlying `dabgent-mcp` Rust MCP server from `../dabgent/dabgent_mcp/`.

## When to Use What

### CLI (main.py)

**Use for:**
- Daily development
- Debugging issues
- Full conversation logging to Postgres
- Custom metrics and instrumentation
- Rapid iteration

**Example:**
```bash
cd /Users/arseni.kravchenko/dev/agent/klaudbiusz
uv run python main.py "Create a simple dashboard"
```

**Features:**
- Full DB logging (run IDs, message tracking, timestamps)
- Token usage and cost tracking
- Conversation replay capabilities
- Direct access to stderr/stdout

### Plugin (.claude-plugin/)

**Use for:**
- Distributing to customers
- Claude Code integration
- End-user workflows
- PoC validation

**Installation:**
```
/plugin marketplace add /Users/arseni.kravchenko/dev/agent/klaudbiusz
/plugin install klaudbiusz
```

Then restart Claude Code.

**Features:**
- Integrated into Claude Code workflow
- Uses Claude Code's API credentials
- Specialized subagent (appbuild) for data app generation
- Standard MCP tool access

## Architecture

```
klaudbiusz/
  main.py              # CLI wrapper (Python)
    ├─> Spawns dabgent-mcp via cargo run
    ├─> Logs to Postgres (optional)
    └─> Uses claude-agent-sdk

  .claude-plugin/
    plugin.json        # Plugin manifest
    README.md          # Plugin installation guide
    └─> Configures Claude Code to spawn dabgent-mcp

Both paths invoke:
  ../dabgent/dabgent_mcp/  (Rust MCP server)
```

## CLI Usage

### Prerequisites

```bash
# Python dependencies
uv sync

# Optional: Postgres DB for logging
export DATABASE_URL="postgresql://..."
```

### Run

```bash
# Basic usage
uv run python main.py "your prompt here"

# Disable DB wiping (keep history)
uv run python main.py "your prompt" --wipe_db=False
```

### Environment Variables

```bash
# Claude API key (required)
export ANTHROPIC_API_KEY="your-key"

# Optional: DB logging
export DATABASE_URL="postgresql://user:pass@host/db"
```

## Plugin Usage

See `.claude-plugin/README.md` for detailed plugin installation and testing instructions.

**Quick start:**
```
/plugin marketplace add /Users/arseni.kravchenko/dev/agent/klaudbiusz
/plugin install klaudbiusz
# Restart Claude Code
```

## Development Workflow

1. **Develop MCP server** in `../dabgent/dabgent_mcp/src/`
2. **Test via CLI** (faster iteration, full logs):
   ```bash
   uv run python main.py "test prompt"
   ```
3. **Validate via plugin** (periodically):
   ```
   /plugin uninstall klaudbiusz
   /plugin install klaudbiusz
   # Restart Claude Code
   ```

## Subagent

The plugin includes the **appbuild** subagent that specializes in data app generation:

- **Invocation**: `@agent-klaudbiusz:appbuild` or automatically when user requests apps/dashboards
- **Workflow**: `initiate_project` → implement with tests → `validate_project`
- **Best practices**: Always adds tests, biases towards backend, validates before completion

The CLI uses similar system prompt customization but without the subagent wrapper.

## MCP Tools Provided

Both distribution methods expose:
- `initiate_project` - Scaffold project structure in ./app/
- `validate_project` - Validate generated code
- Additional tools defined in dabgent-mcp

## Troubleshooting

### CLI Issues

**DB connection fails:**
- Check `DATABASE_URL` is set and valid
- DB is optional, will skip logging if not available

**MCP server fails to start:**
- Verify cargo is in PATH: `which cargo`
- Test MCP build: `cd ../dabgent/dabgent_mcp && cargo build`

### Plugin Issues

**Plugin not found:**
- Verify marketplace path: `/plugin marketplace list`
- Check `.claude-plugin/plugin.json` exists

**Tools not working:**
- Check `/mcp list` shows `klaudbiusz`
- Launch with `claude --mcp-debug` for logs
- Fall back to CLI for debugging

## Future: Binary Distribution

When ready for production:

1. Build dabgent-mcp as static binary:
   ```bash
   cd ../dabgent/dabgent_mcp
   cargo build --release
   ```

2. Update plugin.json to use binary:
   ```json
   "command": "${CLAUDE_PLUGIN_ROOT}/../../dabgent/target/release/dabgent-mcp"
   ```

3. Package and distribute

This eliminates Rust toolchain requirement for users.

## Dependencies

**CLI:**
- Python 3.12+
- uv (package manager)
- claude-agent-sdk
- Optional: asyncpg (for DB logging)

**Plugin:**
- Rust toolchain (cargo)
- dabgent-mcp workspace at `../dabgent/`

**Both require:**
- ANTHROPIC_API_KEY environment variable
