# Evaluation Metrics for Generated Databricks Apps

## Philosophy: Objective Metrics Only

**Goal:** Minimize bias by using only objective, measurable metrics.

**What we measure:**
- ✅ Binary pass/fail (build success, tests pass, etc.)
- ✅ Numeric values (coverage %, LOC, response times)
- ✅ Checklist-based scores (file exists, command defined, etc.)

**What we DON'T measure:**
- ❌ Subjective quality assessments
- ❌ "Is this code good?" judgments
- ❌ "Does this match requirements?" interpretations
- ❌ LLM/VLM scoring of correctness or aesthetics

**Limited AI use:**
- VLM may be used ONLY for binary checks: "Does page render?" (yes/no)
- NO AI-based quality scoring or requirement matching
- Focus on objective "does it work?" not "is it good?"

---

## Top-Level Metrics (Minimal Set)

### 1. BUILD SUCCESS (Binary - Pass/Fail)
**Universal metric for app stack quality**

```bash
# Check: Does the application build without errors?
docker build -t eval-app .
EXIT_CODE=$?
```

**Pass criteria:** Exit code = 0

---

### 2. RUNTIME SUCCESS (Binary - Pass/Fail)
**Universal metric for app stack quality**

```bash
# Check: Does the application start and respond to health checks?
docker run -d -p 8000:8000 --env-file .env --name eval-app eval-app
sleep 10
curl -f http://localhost:8000/healthcheck
EXIT_CODE=$?
```

**Pass criteria:**
- Container starts (doesn't immediately crash)
- Health check returns 200 OK within 30 seconds

---

### 3. TYPE SAFETY (Binary - Pass/Fail)
**Universal metric for app stack quality**

```bash
# Check: Does TypeScript compilation succeed without errors?
cd server && npx tsc --noEmit
cd ../client && npx tsc --noEmit
```

**Pass criteria:** Zero TypeScript errors

---

### 4. TESTS PASS (Binary + Coverage %)
**Universal metric for app stack quality**

```bash
# Check: Do all tests pass? What's the coverage?
cd server && npm test -- --experimental-test-coverage
```

**Pass criteria:**
- All tests pass (0 failures)
- Coverage ≥ 50% (warning if < 70%)

---

### 5. DATABRICKS CONNECTIVITY (Binary - Pass/Fail)
**Databricks-specific metric**

```bash
# Check: Can the app connect to Databricks and execute a simple query?
curl -X POST http://localhost:8000/api/healthcheck \
  -H "Content-Type: application/json" | jq -e '.result'

# Then test first data endpoint
ENDPOINT=$(curl http://localhost:8000/api | jq -r '.procedures[0]')
curl -X POST http://localhost:8000/api/$ENDPOINT \
  -H "Content-Type: application/json" -d '{}' | jq -e '.result.data'
```

**Pass criteria:**
- At least one API endpoint executes
- Returns valid JSON response
- No Databricks authentication errors
- No SQL syntax errors

---

### 6. DATA RETURNED (Binary Check)
**Databricks-specific metric**

**Check: Does the API return data successfully?**

```bash
# Objective check: Can the API execute queries and return results?
curl -X POST http://localhost:8000/api/getData \
  -H "Content-Type: application/json" \
  -d '{}' | jq '.result.data'

# Check:
# - HTTP 200 response
# - Valid JSON structure
# - Non-empty data array
# - No Databricks errors
```

**Pass criteria (objective):**
- ✅ API responds with 200 OK
- ✅ Returns valid JSON
- ✅ Data array is not empty
- ✅ No SQL syntax errors
- ✅ No authentication errors

**Note:** This metric does NOT assess data quality or correctness. It only verifies the app can successfully fetch data from Databricks.

---

### 7. UI RENDERS (Binary Check)
**Universal metric - Objective visual verification**

**Check: Does the app render a page (not blank, not error)?**

```python
# Capture screenshot (already implemented in bulk_run.py)
screenshot_path = capture_screenshot(app_dir)

# Objective VLM check - BINARY ONLY, no quality assessment
vlm_check = evaluate_with_vlm(screenshot_path, """
Look at this screenshot.

Answer ONLY these binary questions:
1. Is the page NOT blank (does something render)? (yes/no)
2. Is there NO error page visible (no 404, 500, crash messages)? (yes/no)
3. Is there ANY visible content (text, tables, charts, etc.)? (yes/no)

Respond with: PASS or FAIL
""")
```

**Pass criteria (objective):**
- ✅ Page is not blank
- ✅ No error messages visible
- ✅ Some content is rendered

**Note:** This metric does NOT assess:
- UI quality or aesthetics
- Whether the visualization is "good"
- Whether data is "correct"
- Whether it matches requirements

It ONLY checks: "Does something appear on screen, and is it not an error page?"

---

### 8. LOCAL RUNABILITY (Score 0-5) - DEVX METRIC
**Developer experience: "Does it just work locally?"**

**Check: How easy is it to run the app locally without Docker?**

```bash
# Check for documentation
[ -f README.md ] && grep -i "getting started\|installation\|setup" README.md

# Check for environment template
[ -f .env.example ] || [ -f .env.template ]

# Check if dependencies install cleanly
cd server && npm install --dry-run
cd ../client && npm install --dry-run

# Check if start command is defined
grep '"start"' server/package.json

# Try to run locally (non-Docker)
cd server && npm install && npm start &
sleep 5
curl -f http://localhost:8000/healthcheck
```

**Scoring criteria (0-5 points):**
- ✅ **+1** README exists with setup instructions
- ✅ **+1** `.env.example` or `.env.template` exists with documented variables
- ✅ **+1** Dependencies install without errors (`npm install` succeeds)
- ✅ **+1** `npm start` command is defined and works
- ✅ **+1** App starts locally within 10 seconds and responds to requests

**Pass criteria:** Score ≥ 3/5

**Why this matters:**
- Developers need to test/debug locally before deploying
- Long setup time = friction = lower adoption
- Missing documentation = guessing game

---

### 9. DEPLOYABILITY (Score 0-5) - DEVX METRIC
**Developer experience: "How production-ready is this?"**

**Check: How easy is it to deploy this application?**

```bash
# Check Dockerfile exists and is optimized
[ -f Dockerfile ] && grep -q "FROM.*alpine" Dockerfile && grep -q "multi-stage" Dockerfile

# Check for deployment configs
[ -f docker-compose.yml ] || [ -f kubernetes.yaml ] || [ -f fly.toml ]

# Check if ports are properly exposed
grep "EXPOSE" Dockerfile

# Check if health checks are defined in Dockerfile
grep "HEALTHCHECK" Dockerfile

# Check if environment variables are templated (not hardcoded)
! grep -r "DATABRICKS_TOKEN=dapi" . --exclude-dir=node_modules

# Check if build script exists
[ -f build.sh ] && [ -x build.sh ]
```

**Scoring criteria (0-5 points):**
- ✅ **+1** Dockerfile exists and builds successfully
- ✅ **+1** Multi-stage build for optimization OR image < 500MB
- ✅ **+1** Health check defined in Dockerfile
- ✅ **+1** Environment variables properly externalized (no secrets in code)
- ✅ **+1** Deployment config exists (docker-compose.yml, k8s, or cloud config)

**Pass criteria:** Score ≥ 3/5

**Why this matters:**
- Production deployment should be one command
- Security: no hardcoded secrets
- Observability: health checks enable monitoring
- Efficiency: optimized images = faster deploys

---

## Evaluation Summary Format

```json
{
  "app_name": "customer-ltv-cohort-analysis",
  "timestamp": "2025-10-17T18:00:00Z",

  "objective_metrics": {
    "build_success": true,
    "runtime_success": true,
    "type_safety": true,
    "tests_pass": true,
    "test_coverage_pct": 68.5,
    "databricks_data_returned": true,
    "ui_renders": true,
    "local_runability_score": 4,
    "deployability_score": 5,
    "total_loc": 1847
  },

  "overall_status": "PASS",
  "score": "9/9 metrics passed",

  "issues": [
    "Test coverage below 70% (68.5%)",
    "Missing .env.example file (local runability)"
  ],

  "metadata": {
    "build_time_sec": 127,
    "image_size_mb": 342,
    "startup_time_sec": 3.2,
    "total_loc": 1847
  }
}
```

---

## Failure Modes & Debugging

| Metric | Common Failures | Debug Actions |
|--------|----------------|---------------|
| **Build Success** | Docker build fails, npm install fails | Check Dockerfile, package.json dependencies |
| **Runtime Success** | Container crashes, port conflict, timeout | Check logs: `docker logs eval-app` |
| **Type Safety** | Type errors, missing types | Run `tsc --noEmit` locally, check generated code |
| **Tests Pass** | Test failures, missing Databricks creds | Run `npm test` with DATABRICKS_* env vars |
| **Databricks Connectivity** | Auth errors, table not found, SQL syntax | Check SQL queries, validate table names exist |
| **Data Validity** | Wrong aggregations, incorrect joins | LLM review + manual SQL inspection |
| **UI Functional** | Blank page, runtime errors, no data | Check browser console logs, network tab |

---

## Implementation Notes

**Automation Sequence:**
```bash
# 1-4: Standard CI/CD checks (fully automated)
./scripts/eval_build.sh app-dir
./scripts/eval_runtime.sh app-dir
./scripts/eval_types.sh app-dir
./scripts/eval_tests.sh app-dir

# 5: Databricks connectivity (requires live DB)
./scripts/eval_databricks.sh app-dir

# 6-7: AI-assisted quality checks
python scripts/eval_data_validity.py app-dir --use-llm
python scripts/eval_ui_functional.py app-dir --use-vlm
```

**Estimated eval time per app:** 3-5 minutes
- Build: 2 min
- Runtime + connectivity: 30 sec
- Tests: 30 sec
- LLM/VLM checks: 1-2 min

**Cost per evaluation:**
- LLM (data validity): ~$0.01 (Claude Haiku)
- VLM (screenshot): ~$0.05 (Claude 3.5 Sonnet)
- **Total: ~$0.06 per app**

---

## Priority Order

If time-constrained, evaluate in this order:

1. ✅ **Build Success** - If this fails, nothing else works
2. ✅ **Runtime Success** - Confirms basic functionality
3. ✅ **Databricks Connectivity** - Confirms core feature works
4. ✅ **UI Functional (VLM)** - Confirms end-to-end works
5. ⚠️ **Type Safety** - Quality gate
6. ⚠️ **Tests Pass** - Quality gate
7. ℹ️ **Data Validity (LLM)** - Deep quality check

**Minimum viable evaluation:** Metrics 1-4 only (no AI needed)
**Production-grade evaluation:** All 9 metrics

---

## DORA Metrics Integration

See `DORA_METRICS.md` for detailed analysis of how these metrics support DORA (DevOps Research and Assessment) performance indicators.

**Current DORA coverage:**

| DORA Metric | Supported By | Coverage |
|-------------|--------------|----------|
| **Deployment Frequency** | Build Success, Runtime Success, Deployability Score | 🟡 Enables frequent deployment, but doesn't track actual frequency |
| **Lead Time for Changes** | Build Time (tracked), Generation Time (tracked) | 🟡 60% - Missing deployment phase tracking |
| **Mean Time to Recovery** | Health Checks, Runtime Success | 🔴 20% - Need observability & incident tracking |
| **Change Failure Rate** | All 9 metrics (pre-deployment validation) | 🟢 70% - Strong pre-deploy, weak post-deploy tracking |

**Legend:**
- 🟢 Good coverage (>60%)
- 🟡 Partial coverage (30-60%)
- 🔴 Needs work (<30%)
