# MLflow Integration for Evaluation Tracking

This document explains how Klaudbiusz uses Databricks Managed MLflow to track evaluation runs, metrics, and quality trends over time.

## Overview

Klaudbiusz now integrates with **Databricks Managed MLflow** to provide comprehensive tracking of:

- **Evaluation Runs**: Each evaluation as a tracked MLflow run
- **Parameters**: Mode (MCP/Vanilla), timestamp, app count, model version
- **Metrics**: Success rates, quality scores, and aggregate statistics
- **Artifacts**: Evaluation reports (JSON and Markdown)
- **Trends**: Track quality improvements/regressions over time

## Benefits

### 1. **Historical Tracking**
- Monitor code generation quality across multiple runs
- Compare MCP mode vs Vanilla SDK mode performance
- Track trends in build success, test pass rates, etc.

### 2. **Reproducibility**
- Every evaluation run is logged with full parameters
- Link back to specific model versions and configurations
- Reproduce results from any past evaluation

### 3. **Quality Monitoring**
- Set up alerts for quality regressions
- Track overall quality score over time
- Identify which metrics are improving vs degrading

### 4. **Cost Analysis**
- Track generation cost per app over time
- Monitor efficiency (apps per dollar)
- Compare cost across different modes

## Setup

### Prerequisites

MLflow tracking requires Databricks credentials:

```bash
export DATABRICKS_HOST=https://your-workspace.databricks.com
export DATABRICKS_TOKEN=dapi...
export ANTHROPIC_API_KEY=sk-ant-...
```

Or create a `.env` file:

```bash
DATABRICKS_HOST=https://your-workspace.databricks.com
DATABRICKS_TOKEN=dapi...
ANTHROPIC_API_KEY=sk-ant-...
```

### Install Dependencies

```bash
# Install with uv
uv sync

# Or with pip
pip install mlflow>=2.15.0
```

## Usage

### Automatic Tracking (Recommended)

The easiest way is to use the run scripts, which automatically enable MLflow tracking:

```bash
# Run with automatic MLflow tracking
./run_vanilla_eval.sh

# Or MCP mode
./run_mcp_eval.sh

# Or both for comparison
./run_all_evals.sh
```

MLflow tracking happens automatically after evaluation completes.

### Manual Tracking

If running evaluations manually:

```bash
# Set the mode for MLflow tracking
export EVAL_MODE="manual"

# Run evaluation
python3 evaluate_apps.py

# MLflow tracking happens automatically at the end
```

### Viewing Results

#### In Databricks UI

1. Navigate to your Databricks workspace
2. Go to **Machine Learning** → **Experiments**
3. Find experiment: `/Shared/klaudbiusz-evaluations`
4. Browse runs, compare metrics, view artifacts

#### Command Line Comparison

Use the comparison utility to view runs from the terminal:

```bash
# Compare recent runs
uv run cli/mlflow_compare.py

# Or with python
python3 cli/mlflow_compare.py
```

Example output:

```
MLflow Evaluation Run Comparison
==================================
Run Name                              Mode           Date
-------------------------------------------------------------------------------------------------
eval_mcp_2025-10-22T12:34:56         mcp            2025-10-22 12:34:56
eval_vanilla_sdk_2025-10-22T11:20:30 vanilla_sdk    2025-10-22 11:20:30

Metrics Comparison
==================================
Metric                                eval_mcp_202  eval_vanilla
-------------------------------------------------------------------------------------------------
build_success_rate                       100.00%      100.00%
runtime_success_rate                       0.00%        0.00%
tests_pass_rate                           85.00%       85.00%
databricks_connectivity_rate              90.00%       90.00%
avg_local_runability_score                 2.00         2.00
avg_deployability_score                    3.00         3.00
overall_quality_score                      0.90         0.90

Latest vs Previous Run
==================================
Metric                                Latest      Previous    Change
-------------------------------------------------------------------------------------------------
build_success_rate                   100.00%      100.00%      +0.0%
databricks_connectivity_rate          90.00%       85.00%      +5.9%
avg_local_runability_score             2.00         2.00       +0.0%
```

## Tracked Metrics

### Success Rate Metrics

These track the percentage of apps passing each criterion:

- `build_success_rate`: Percentage of apps that build successfully
- `runtime_success_rate`: Percentage of apps that start without crashing
- `tests_pass_rate`: Percentage of apps with passing tests
- `databricks_connectivity_rate`: Percentage with proper DB connectivity

### Aggregate Metrics

- `total_apps`: Number of apps evaluated
- `evaluated`: Number successfully evaluated
- `avg_local_runability_score`: Average score (0-5) for local development setup
- `avg_deployability_score`: Average score (0-5) for deployment readiness
- `overall_quality_score`: Composite quality score (0-1)

### Generation Metrics (Future)

When integrated with bulk_run.py:

- `generation_cost_usd`: Total cost to generate all apps
- `total_output_tokens`: Total tokens generated
- `avg_turns_per_app`: Average conversation turns per app
- `apps_per_dollar`: Efficiency metric (apps / cost)

## Tracked Parameters

Every run logs these parameters:

- `mode`: Generation mode (mcp, vanilla_sdk, manual)
- `total_apps`: Number of apps evaluated
- `timestamp`: ISO 8601 timestamp of evaluation
- `model_version`: Claude model version used (e.g., claude-sonnet-4-5-20250929)

## Tracked Artifacts

Each run includes these artifacts:

- `evaluation_report.json`: Full structured evaluation data
- `EVALUATION_REPORT.md`: Human-readable markdown report

## MLflow Experiment Structure

**Experiment Name**: `/Shared/klaudbiusz-evaluations`

**Run Naming Convention**: `eval_{mode}_{timestamp}`
- Example: `eval_mcp_2025-10-22T12:34:56Z`
- Example: `eval_vanilla_sdk_2025-10-22T11:20:30Z`

**Tags**:
- `framework`: Always "klaudbiusz"
- `mode`: Generation mode (mcp, vanilla_sdk, etc.)
- `run_name`: Friendly name for the run

## Architecture

### Components

1. **mlflow_tracker.py**: Core MLflow integration module
   - `EvaluationTracker`: Main class for tracking runs
   - `track_evaluation()`: Convenience function for quick tracking

2. **evaluate_apps.py**: Evaluation script with MLflow integration
   - Automatically tracks after generating reports
   - Falls back gracefully if MLflow unavailable

3. **mlflow_compare.py**: Comparison utility
   - Compare recent runs side-by-side
   - Show trends and changes
   - Group by mode for comparison

4. **Run Scripts**: Set `EVAL_MODE` environment variable
   - `run_vanilla_eval.sh`: Sets `EVAL_MODE=vanilla_sdk`
   - `run_mcp_eval.sh`: Sets `EVAL_MODE=mcp`

### Data Flow

```
1. Run evaluation script
   ↓
2. Generate evaluation_report.json
   ↓
3. MLflow tracker initialized
   ↓
4. Start MLflow run with parameters
   ↓
5. Log evaluation metrics
   ↓
6. Log report artifacts
   ↓
7. End MLflow run
   ↓
8. Results viewable in Databricks UI
```

## Advanced Usage

### Programmatic Access

```python
from cli.mlflow_tracker import EvaluationTracker

# Initialize tracker
tracker = EvaluationTracker(experiment_name="/Shared/my-experiment")

# Start a run
run_id = tracker.start_run("my_run", tags={"version": "1.0"})

# Log parameters
tracker.log_evaluation_parameters(
    mode="custom",
    total_apps=20,
    timestamp="2025-10-22T12:00:00Z"
)

# Log metrics
tracker.log_evaluation_metrics(evaluation_report)

# Log artifacts
tracker.log_artifact_file("report.json")

# End run
tracker.end_run()
```

### Comparing Specific Runs

```python
from cli.mlflow_tracker import EvaluationTracker

tracker = EvaluationTracker()

# Compare two specific run IDs
comparison = tracker.compare_runs([run_id_1, run_id_2])

for run_id, data in comparison.items():
    print(f"Run {run_id}:")
    print(f"  Metrics: {data['metrics']}")
    print(f"  Params: {data['params']}")
```

### Custom Metrics

Add custom metrics to your evaluation:

```python
tracker = EvaluationTracker()
tracker.start_run("custom_metrics_run")

# Log custom metrics
import mlflow
mlflow.log_metric("custom_score", 0.95)
mlflow.log_metric("my_metric", 42)

tracker.end_run()
```

## Troubleshooting

### MLflow Tracking Disabled

If you see:
```
⚠️  MLflow tracking disabled: DATABRICKS_HOST or DATABRICKS_TOKEN not set
```

**Solution**: Set environment variables:
```bash
export DATABRICKS_HOST=https://your-workspace.databricks.com
export DATABRICKS_TOKEN=dapi...
```

### MLflow Setup Failed

If you see:
```
⚠️  MLflow setup failed: [error message]
```

**Common causes**:
1. Invalid Databricks credentials
2. Network connectivity issues
3. Missing mlflow package

**Solution**:
```bash
# Test Databricks connection
curl -H "Authorization: Bearer $DATABRICKS_TOKEN" $DATABRICKS_HOST/api/2.0/clusters/list

# Install/update mlflow
uv add mlflow
# or
pip install --upgrade mlflow
```

### Cannot Find Experiment

If experiment `/Shared/klaudbiusz-evaluations` doesn't exist:

**Solution**: The tracker will create it automatically on first run. If it fails, create manually in Databricks UI:
1. Go to Machine Learning → Experiments
2. Click "Create Experiment"
3. Name: `/Shared/klaudbiusz-evaluations`

## Best Practices

### 1. Always Set EVAL_MODE

When running evaluations, set the mode to distinguish runs:

```bash
export EVAL_MODE="mcp"  # or "vanilla_sdk", "manual", "experimental", etc.
python3 evaluate_apps.py
```

### 2. Use Descriptive Run Names

For manual runs, use descriptive names:

```python
tracker.start_run("eval_with_new_prompts_v2", tags={"experiment": "prompt_optimization"})
```

### 3. Tag Important Runs

Add tags to mark significant runs:

```python
tracker.start_run(run_name, tags={
    "production_candidate": "true",
    "git_commit": "abc123",
    "experiment_phase": "optimization"
})
```

### 4. Regular Comparison

Run comparisons regularly to spot trends:

```bash
# Weekly check
python3 cli/mlflow_compare.py
```

### 5. Archive Old Runs

In Databricks UI, archive old runs to keep experiment clean:
- Keep successful production runs
- Archive failed or experimental runs

## Future Enhancements

Planned improvements:

1. **Generation Metrics Integration**: Track cost, tokens, turns from bulk_run.py
2. **Automated Alerts**: Email/Slack notifications on quality regressions
3. **Dashboard**: Custom Streamlit dashboard for visualization
4. **A/B Testing**: Compare different generation strategies
5. **Model Registry**: Register high-quality app generators

## Related Documentation

- [README.md](README.md) - Main project documentation
- [eval-docs/evals.md](eval-docs/evals.md) - Evaluation framework details
- [eval-docs/EVALUATION_METHODOLOGY.md](eval-docs/EVALUATION_METHODOLOGY.md) - Zero-bias methodology
- [Databricks MLflow Docs](https://docs.databricks.com/mlflow/) - Official MLflow documentation

## Support

For issues or questions:
- Check troubleshooting section above
- Review MLflow tracker code: `cli/mlflow_tracker.py`
- Check Databricks MLflow UI for run details

---

**Version**: 1.0.0
**Last Updated**: 2025-10-22
**MLflow Version**: ≥2.15.0
