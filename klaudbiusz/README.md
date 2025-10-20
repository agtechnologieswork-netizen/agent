# Klaudbiusz

AI-powered Databricks application generator with objective evaluation framework.

## Overview

Klaudbiusz generates production-ready Databricks applications from natural language prompts and evaluates them using 9 objective, zero-bias metrics. This enables autonomous deployment workflows where AI-generated code can be automatically validated and deployed without human review.

**Current Results:** 90% of generated apps (18/20) are production-ready and deployable.

## Quick Start

### Generate Applications

```bash
cd klaudbiusz
export DATABRICKS_HOST=https://your-workspace.databricks.com
export DATABRICKS_TOKEN=dapi...
export ANTHROPIC_API_KEY=sk-ant-...

# Generate a single app
uv run cli/main.py "Create a customer churn analysis dashboard"

# Batch generate from prompts
uv run cli/bulk_run.py
```

### Evaluate Generated Apps

```bash
cd cli
export DATABRICKS_HOST=https://your-workspace.databricks.com
export DATABRICKS_TOKEN=dapi...

# Evaluate all apps
python3 evaluate_all.py

# Evaluate single app
python3 evaluate_app.py ../app/customer-churn-analysis
```

## Evaluation Framework

We use **9 objective metrics** to measure autonomous deployability:

| Category | Metrics | Current Results |
|----------|---------|----------------|
| **Core Functionality** | Build, Runtime, Type Safety, Tests | 90%, 90%, 0%, 0% |
| **Databricks Integration** | DB Connectivity, Data Returned | 90%, 0% |
| **UI** | UI Renders | 0% |
| **Agentic DevX** | Local Runability, Deployability | 3.0/5, 3.0/5 |

**See [evals.md](evals.md) for complete metric definitions.**

### Key Innovation: Agentic DevX

We measure **whether an AI agent can autonomously run and deploy the code** with zero configuration:

- **Local Runability:** Can run with `npm install && npm start`? (3.0/5)
- **Deployability:** Can deploy with `docker build && docker run`? (3.0/5)

**See [DORA_METRICS.md](DORA_METRICS.md) for detailed agentic evaluation approach.**

## Documentation

### Framework & Methodology
- **[evals.md](evals.md)** - Complete 9-metric framework definition
- **[EVALUATION_METHODOLOGY.md](EVALUATION_METHODOLOGY.md)** - Zero-bias evaluation methodology
- **[DORA_METRICS.md](DORA_METRICS.md)** - DORA metrics integration & agentic DevX

## Project Structure

```
klaudbiusz/
├── README.md                    # This file
├── app/                         # Generated applications (gitignored)
├── cli/                         # Generation & evaluation scripts
│   ├── bulk_run.py             # Batch app generation
│   ├── evaluate_all.py         # Batch evaluation
│   ├── evaluate_app.py         # Single app evaluation
│   ├── archive_evaluation.sh   # Create evaluation archive
│   └── cleanup_evaluation.sh   # Clean generated apps
├── evals.md                    # Metric definitions
├── DORA_METRICS.md             # DORA & agentic DevX
├── EVALUATION_METHODOLOGY.md   # Zero-bias approach
├── EVALUATION_REPORT.md        # Latest results (gitignored)
└── evaluation_report.*         # Latest data (gitignored)
```

## Workflows

### Development Workflow

1. Write natural language prompt
2. Generate: `uv run cli/bulk_run.py`
3. Evaluate: `python3 cli/evaluate_all.py`
4. Review: `cat EVALUATION_REPORT.md`
5. Deploy apps that pass checks

### Archive & Clean Workflow

```bash
# Create archive of apps + reports
./cli/archive_evaluation.sh

# Verify checksum
shasum -a 256 -c klaudbiusz_evaluation_*.tar.gz.sha256

# Clean up generated apps
./cli/cleanup_evaluation.sh
```

## Requirements

- Python 3.11+
- uv (Python package manager)
- Docker (for builds and runtime checks)
- Node.js 18+ (for generated apps)
- Databricks workspace with access token

## Environment Variables

```bash
# Required for generation
export DATABRICKS_HOST=https://your-workspace.databricks.com
export DATABRICKS_TOKEN=dapi...
export ANTHROPIC_API_KEY=sk-ant-...

# Optional for logging
export DATABASE_URL=postgresql://...
```

## Core Principle

> If an AI agent cannot autonomously deploy its own generated code, that code is not production-ready.

All metrics are **objective, reproducible, and automatable** - no subjective quality assessments.

**See [EVALUATION_METHODOLOGY.md](EVALUATION_METHODOLOGY.md) for our zero-bias philosophy.**

## License

Apache 2.0
