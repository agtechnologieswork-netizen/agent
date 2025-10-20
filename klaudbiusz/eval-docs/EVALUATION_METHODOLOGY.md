# Evaluation Methodology: Zero-Bias Approach

## Core Principle

**Goal:** Evaluate generated applications using only objective, measurable criteria to eliminate subjective bias.

> Traditional evaluation asks: "Is this code well-written?" (subjective)
>
> Our approach asks: "Can an agent deploy this autonomously?" (objective)

---

## What We Measure vs. Don't Measure

### ✅ Allowed: Objective Measurements

1. **Binary Pass/Fail** - Exit codes, HTTP status codes, yes/no checks
2. **Numeric Values** - Coverage %, LOC, response times, memory usage
3. **Checklist Scores** - File exists, command defined, endpoint responds

### ❌ Prohibited: Subjective Assessments

1. **Quality Judgments** - "Is this code good?", "Is the UI attractive?"
2. **Requirement Matching** - "Does this match the prompt?", "Is this what the user wanted?"
3. **LLM/VLM Scoring** - "Rate this 1-10", "Score the quality", "Assess correctness"

### Limited AI Use

**VLM allowed ONLY for binary checks:**
- ✅ "Does the page render?" → yes/no
- ✅ "Is there an error visible?" → yes/no

**NOT allowed:**
- ❌ "Rate the UI quality"
- ❌ "Does this match requirements?"

---

## The 9 Metrics

**See [evals.md](evals.md) for complete metric definitions with implementation details.**

Summary:
1. **Build Success** - Does `docker build` succeed?
2. **Runtime Success** - Does container start + health check respond?
3. **Type Safety** - Does `tsc --noEmit` pass?
4. **Tests Pass** - Does `npm test` pass? What's coverage?
5. **DB Connectivity** - Can app connect to Databricks?
6. **Data Returned** - Do API endpoints return data?
7. **UI Renders** - Does frontend load without errors?
8. **Local Runability** - Can run with `npm install && npm start`? (0-5)
9. **Deployability** - Can deploy with `docker build && docker run`? (0-5)

---

## Output Format: CSV Schema

All evaluation results are exported to CSV with the following columns:

```csv
app_name,timestamp,build_success,runtime_success,type_safety_pass,tests_pass,
test_coverage_pct,databricks_connectivity,data_returned,ui_renders,
local_runability_score,deployability_score,build_time_sec,startup_time_sec,
total_loc,has_dockerfile,has_tests,issue_count,issues
```

### Column Definitions

**Binary Metrics (0 or 1):**
- `build_success`, `runtime_success`, `type_safety_pass`, `tests_pass`
- `databricks_connectivity`, `data_returned`, `ui_renders`
- `has_dockerfile`, `has_tests`

**Numeric Metrics:**
- `test_coverage_pct` - Percentage (0.0-100.0)
- `local_runability_score` - Score (0-5)
- `deployability_score` - Score (0-5)
- `build_time_sec` - Seconds (float)
- `startup_time_sec` - Seconds (float)
- `total_loc` - Lines of code (integer)
- `issue_count` - Number of issues (integer)

**Text Fields:**
- `app_name` - Application identifier
- `timestamp` - ISO 8601 timestamp
- `issues` - Semicolon-separated issue list

---

## Reproducibility Requirements

All metrics must be **100% reproducible**:

### Same Input → Same Output
- Same app code → Same evaluation scores
- No randomness in evaluation
- No human interpretation required

### Automatable
- Can run in CI/CD without human input
- Exit codes and numeric values only
- No manual verification steps

### Trackable Over Time
- Metrics can be compared across runs
- Changes are measurable (not "better" or "worse", but "+10%" or "-2 issues")
- Trends are numeric, not subjective

---

## Bias Minimization Techniques

### 1. No LLM Quality Scoring
- ❌ "Rate this code 1-5"
- ❌ "Assess if this meets requirements"
- ✅ Binary: does it build? does it run?

### 2. No Subjective Thresholds
- ❌ "Code is good if coverage > X%"
- ✅ Report actual coverage, let user decide threshold

### 3. No Prompt Matching
- ❌ "Does this dashboard match the user's request?"
- ✅ "Does the API return data?" (yes/no)

### 4. Checklist-Based Scores Only
- Local Runability: +1 for each: package.json, start script, deps install, .env.example, README
- Deployability: +1 for each: Dockerfile, build succeeds, container starts, health check, docs
- No interpretation of "quality" - just presence/absence

---

## Why This Matters

### For AI-Generated Code

AI code generators should produce **autonomously deployable** applications. If a human needs to review/fix/interpret, the automation has failed.

Our metrics answer: "Can an AI agent deploy this code without human help?"

### For Continuous Improvement

Objective metrics enable:
- **A/B testing** - Compare generation approaches numerically
- **Regression detection** - Alert when metrics drop
- **Trend analysis** - Track improvement over time
- **Benchmarking** - Compare against industry standards

### For DORA Metrics

Our objective metrics directly enable DORA tracking:
- **Deployment Frequency** - % of apps that pass deployment checks
- **Lead Time** - Generation time + build time (measurable)
- **Change Failure Rate** - % failing build/runtime (measurable)
- **MTTR** - Container restart time (measurable)

**See [DORA_METRICS.md](DORA_METRICS.md) for complete DORA integration details.**

---

## Implementation Notes

### Running Evaluations

```bash
# Single app
python3 cli/evaluate_app.py app/my-app

# All apps
python3 cli/evaluate_all.py

# Outputs
# - evaluation_report.json (structured data)
# - evaluation_report.csv (spreadsheet)
# - EVALUATION_REPORT.md (human-readable)
```

### Evaluation Scripts

All scripts in `cli/`:
- `evaluate_app.py` - Single app evaluation (all 9 metrics)
- `evaluate_all.py` - Batch evaluation across all apps
- `compare_evaluations.py` - Compare two evaluation runs

### Docker-Based Validation

All build/runtime checks use Docker to ensure:
- Consistent environment across evaluations
- No dependency on local Node.js/Python versions
- Reproducible results on any machine

---

## Future Enhancements

### Metric 6: Data Returned
- Currently not implemented (requires app-specific tRPC procedure names)
- Planned: Call first data endpoint, check for HTTP 200 + JSON response

### Metric 7: UI Renders
- Currently not implemented (requires Playwright + VLM)
- Planned: Screenshot + binary VLM check "Does page render?"

### Observability Score (New)
- Track logging, error reporting, metrics instrumentation
- Enable MTTR measurement

---

**Last Updated:** October 17, 2025
**Framework Version:** 1.0 (9 metrics, zero-bias approach)
