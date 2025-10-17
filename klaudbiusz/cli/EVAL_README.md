# App Evaluation Script

Simple automated evaluation of generated Databricks apps based on the 7 core metrics.

## Quick Start

```bash
# Install dependencies
pip install anthropic

# Set required environment variables
export DATABRICKS_HOST=https://your-workspace.databricks.com
export DATABRICKS_TOKEN=dapi...
export ANTHROPIC_API_KEY=sk-ant-...

# Evaluate a single app
cd klaudbiusz/cli
python evaluate_app.py ../app/customer-ltv-cohort-analysis

# Evaluate all apps
python evaluate_app.py --all
```

## What It Checks

### Automated Checks (No AI)
1. **Build Success** - Docker build completes
2. **Runtime Success** - Container starts, health check responds
3. **Type Safety** - TypeScript compiles without errors
4. **Tests Pass** - All tests pass, coverage measured
5. **Databricks Connectivity** - API endpoints work, queries execute

### AI-Assisted Checks
6. **Data Validity (LLM)** - Claude Haiku validates SQL logic (~$0.01/app)
7. **UI Functional (VLM)** - Claude Sonnet validates UI screenshot (~$0.05/app)

## Output

Results are saved as JSON:

```json
{
  "app_name": "customer-ltv-cohort-analysis",
  "timestamp": "2025-10-17T18:30:00Z",
  "overall_status": "PASS",
  "metrics": {
    "build_success": true,
    "runtime_success": true,
    "type_safety": true,
    "tests_pass": true,
    "test_coverage_pct": 68.5,
    "databricks_connectivity": true,
    "data_validity_score": 5,
    "ui_functional_score": 4
  },
  "issues": [
    "Test coverage below 70% (68.5%)"
  ],
  "metadata": {
    "build_time_sec": 127,
    "startup_time_sec": 3.2,
    "total_loc": 1847
  }
}
```

## Features

- **Reuses existing screenshots** - Leverages screenshots already captured by `bulk_run.py`
- **Loads prompts automatically** - Reads from `bulk_run_results_*.json` files
- **Automatic cleanup** - Stops and removes Docker containers
- **Fast evaluation** - 3-5 minutes per app
- **Low cost** - ~$0.06 per app for AI checks

## Requirements

- Docker
- Node.js 20+
- Python 3.10+
- `anthropic` Python package
- Environment variables:
  - `DATABRICKS_HOST`
  - `DATABRICKS_TOKEN`
  - `ANTHROPIC_API_KEY`

## Skip AI Checks

The script works without AI if `anthropic` package or API key is missing:
- Metrics 6-7 will be skipped
- Overall evaluation still runs on metrics 1-5
- Critical checks (build, runtime, connectivity) still enforced

## Batch Evaluation

```bash
# Evaluate all apps and save combined results
python evaluate_app.py --all

# Output: eval_results_<timestamp>.json with array of all results
```

## Integration with CI/CD

```yaml
# .github/workflows/eval.yml
- name: Evaluate app
  run: |
    python klaudbiusz/cli/evaluate_app.py klaudbiusz/app/${{ matrix.app }}
  env:
    DATABRICKS_HOST: ${{ secrets.DATABRICKS_HOST }}
    DATABRICKS_TOKEN: ${{ secrets.DATABRICKS_TOKEN }}
    ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
```
