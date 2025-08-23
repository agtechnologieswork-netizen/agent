# Trace Analysis Scripts

## Quick Start

```bash
# Process all traces in a directory (organized by agent type, 8 workers)
uv run python analysis/process_all_traces.py /path/to/traces output_dir

# Process with custom worker count
uv run python analysis/process_all_traces.py /path/to/traces output_dir --max_workers 16

# View single trace
uv run python analysis/trace_processor.py trace.json output.txt

# Analyze processed traces with Gemini
uv run python analysis/analyze_traces_with_gemini.py /path/to/processed_txt_files
```

## Scripts

- **`process_all_traces.py`** - Batch process FSM traces with 8 concurrent workers, organizes output by agent type into separate directories
- **`trace_processor.py`** - Process single trace file, works with all FSM agents
- **`analyze_traces_with_gemini.py`** - Takes directory of processed .txt files, concatenates and sends to Gemini for analysis
- **`nicegui_trace_viewer.py`** - Legacy name, now works with all FSM agents

## Output Structure

`process_all_traces.py` creates organized directories:
```
output_dir/
├── trpc_agent/       # tRPC agent traces
├── laravel_agent/    # Laravel agent traces  
├── nicegui_agent/    # NiceGUI agent traces
└── unknown/          # Unclassified traces (should be empty)
```

## Supported Agents

✅ `trpc_agent`, `nicegui_agent`, `laravel_agent`  
❌ `template_diff` (not FSM-based)

## Workflow Example

```bash
# 1. Process traces (organized by agent type)
uv run python analysis/process_all_traces.py /path/to/raw_traces /tmp/organized

# 2. Analyze specific agent type with Gemini
uv run python analysis/analyze_traces_with_gemini.py /tmp/organized/laravel_agent

# Output: /tmp/organized/laravel_agent/gemini_analysis.md
```