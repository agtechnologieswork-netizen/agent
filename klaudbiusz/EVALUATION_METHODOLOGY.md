# Evaluation Methodology: Objective Metrics Only

## Core Principle: Bias Minimization

**Goal:** Evaluate generated applications using only objective, measurable criteria to minimize subjective bias.

### What We Measure (✅ Allowed)

1. **Binary Pass/Fail**
   - Does the code compile? (yes/no)
   - Do tests pass? (yes/no)
   - Does the container build? (yes/no)
   - Does the API return data? (yes/no)

2. **Numeric Measurements**
   - Test coverage percentage
   - Lines of code
   - Build time (seconds)
   - Response time (milliseconds)
   - Memory usage (MB)

3. **Checklist-Based Scores**
   - File exists (yes/no)
   - Command defined (yes/no)
   - Documentation present (yes/no)
   - Environment template exists (yes/no)
   - Multi-stage Dockerfile (yes/no)

### What We DON'T Measure (❌ Prohibited)

1. **Subjective Quality**
   - "Is this code good?"
   - "Is the UI attractive?"
   - "Does this look professional?"
   - "Is the data visualization effective?"

2. **Requirement Matching**
   - "Does this match the prompt?"
   - "Is this what the user wanted?"
   - "Are the aggregations correct for this use case?"

3. **LLM/VLM Scoring**
   - ❌ NO: "Rate this SQL query 1-5"
   - ❌ NO: "Score the UI quality"
   - ❌ NO: "Assess if this meets requirements"

### Limited AI Use (Allowed)

**VLM for Binary Checks ONLY:**
- ✅ "Does the page render (not blank)?" → yes/no
- ✅ "Is there an error message visible?" → yes/no
- ✅ "Is there ANY content on screen?" → yes/no

**NOT allowed:**
- ❌ "Rate the UI 1-10"
- ❌ "Does this visualization match the prompt?"
- ❌ "Is this dashboard well-designed?"

---

## 9 Objective Metrics

### 1. Build Success (Binary)
- **Measurement:** `docker build` exit code
- **Pass:** Exit code = 0
- **Fail:** Exit code ≠ 0
- **No interpretation required**

### 2. Runtime Success (Binary)
- **Measurement:** Container starts + health check responds
- **Pass:** HTTP 200 from /healthcheck within 30s
- **Fail:** Container crashes or no response
- **No interpretation required**

### 3. Type Safety (Binary)
- **Measurement:** `tsc --noEmit` exit code
- **Pass:** Zero TypeScript errors
- **Fail:** One or more errors
- **No interpretation required**

### 4. Tests Pass (Binary + Coverage %)
- **Measurement:** `npm test` exit code + coverage report
- **Pass:** All tests pass
- **Fail:** Any test fails
- **Coverage:** Numeric value from test runner
- **No interpretation required**

### 5. Databricks Connectivity (Binary)
- **Measurement:** API endpoint returns data
- **Pass:** HTTP 200 + non-empty JSON array
- **Fail:** Error response or empty data
- **Does NOT assess data correctness**

### 6. Data Returned (Binary)
- **Measurement:** API returns data without errors
- **Pass:** HTTP 200 + valid JSON + no SQL errors
- **Fail:** HTTP error or SQL error
- **Does NOT assess if data is "correct" for the prompt**

### 7. UI Renders (Binary)
- **Measurement:** Screenshot shows content
- **Pass:** Page not blank + no error page + content visible
- **Fail:** Blank page or error page
- **Does NOT assess UI quality or correctness**

### 8. Local Runability (Score 0-5)
- **Measurement:** Checklist of 5 items
  - README with setup instructions (0/1)
  - .env.example exists (0/1)
  - Dependencies defined (0/1)
  - npm start command (0/1)
  - Entry point exists (0/1)
- **Score:** Sum of checklist items
- **No interpretation required**

### 9. Deployability (Score 0-5)
- **Measurement:** Checklist of 5 items
  - Dockerfile exists (0/1)
  - Multi-stage or Alpine (0/1)
  - HEALTHCHECK directive (0/1)
  - EXPOSE directive (0/1)
  - No hardcoded secrets (0/1)
- **Score:** Sum of checklist items
- **No interpretation required**

---

## Output Formats

### 1. JSON (Structured Data)
- **File:** `evaluation_report.json`
- **Contains:** Full metrics for all apps + summary statistics
- **Use:** Programmatic analysis, API integration

### 2. Markdown (Human-Readable Report)
- **File:** `EVALUATION_REPORT.md`
- **Contains:** Summary tables, recommendations, detailed breakdown
- **Use:** Code review, team communication

### 3. CSV (Spreadsheet Analysis)
- **File:** `evaluation_report.csv`
- **Contains:** One row per app, all metrics as columns
- **Use:** Excel analysis, data visualization, trend tracking

**CSV Schema:**
```csv
app_name,timestamp,type_safety_pass,tests_pass,test_coverage_pct,local_runability_score,deployability_score,total_loc,has_dockerfile,has_tests,issue_count,issues
```

---

## Why This Matters

### Problem: Subjective Bias
- LLM scoring "Is this SQL good?" is subjective
- "Does this match requirements?" depends on interpretation
- "Is the UI professional?" varies by evaluator

### Solution: Objective Measures
- "Does the code compile?" is objective
- "Do tests pass?" is objective
- "Is coverage > 70%?" is objective

### Result: Reproducible Evaluation
- Same app → same scores (deterministic)
- No evaluator bias
- Clear pass/fail criteria
- Trackable over time

---

## Usage

### Run Evaluation
```bash
cd klaudbiusz/cli
source /path/to/.venv/bin/activate
python evaluate_all.py
```

### Output Files
- `evaluation_report.json` - Machine-readable
- `EVALUATION_REPORT.md` - Human-readable
- `evaluation_report.csv` - Spreadsheet-ready

### Example CSV Output
```csv
app_name,type_safety_pass,tests_pass,test_coverage_pct,local_runability_score,deployability_score
customer-ltv-cohort-analysis,0,0,0.0,3,3
churn-risk-dashboard,0,0,0.0,3,3
```

---

## Future: DORA Metrics Integration

Once we have objective baseline metrics, we can track:

1. **Deployment Frequency** - Count of successful builds
2. **Lead Time** - Time from generation to "all metrics pass"
3. **Change Failure Rate** - % of apps that fail critical checks
4. **MTTR** - Time to fix failing metrics

All still objective, measurable, bias-free.

---

## FAQ

**Q: Why not use LLM to check if SQL matches the prompt?**
A: That's subjective interpretation. We measure "does it return data?" (objective), not "is it the right data?" (subjective).

**Q: Why not assess UI quality?**
A: Quality is subjective. We measure "does something render?" (objective), not "is it pretty?" (subjective).

**Q: How do we know if the app is actually good?**
A: We measure if it works (builds, runs, tests pass). Functional correctness comes from human review, not automated metrics.

**Q: Can we add requirement matching later?**
A: Maybe, but only if we can define objective criteria (e.g., "returns data for time period X"). Never "does this feel right?"

---

## Summary

**Measure:**
- ✅ Does it build?
- ✅ Does it run?
- ✅ Do tests pass?
- ✅ Does it return data?
- ✅ Does UI render?

**Don't Measure:**
- ❌ Is it good?
- ❌ Is it correct?
- ❌ Does it match requirements?
- ❌ Would users like this?

**Result:** Reproducible, bias-free, objective evaluation.
