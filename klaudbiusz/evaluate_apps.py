#!/usr/bin/env python3
"""
Evaluate all apps in the app/ directory using the 9-metric framework.
"""
import os
import json
import subprocess
import time
from pathlib import Path
from typing import Dict, List, Any, Optional

APP_DIR = Path("app")
RESULTS = []


def run_command(cmd: str, cwd: str, timeout: int = 60) -> tuple[int, str, str]:
    """Run a command and return exit code, stdout, stderr."""
    try:
        result = subprocess.run(
            cmd,
            shell=True,
            cwd=cwd,
            capture_output=True,
            text=True,
            timeout=timeout
        )
        return result.returncode, result.stdout, result.stderr
    except subprocess.TimeoutExpired:
        return -1, "", "Command timed out"
    except Exception as e:
        return -1, "", str(e)


def check_file_exists(app_path: Path, filename: str) -> bool:
    """Check if a file exists in the app directory."""
    return (app_path / filename).exists()


def evaluate_build_success(app_path: Path, app_name: str) -> tuple[bool, str]:
    """Metric 1: BUILD SUCCESS (Binary)"""
    # Check for different build systems
    if check_file_exists(app_path, "package.json"):
        # Node.js project
        code, stdout, stderr = run_command("npm install", str(app_path), timeout=120)
        if code != 0:
            return False, f"npm install failed: {stderr[:200]}"

        # Check if there's a build script
        try:
            with open(app_path / "package.json") as f:
                pkg = json.load(f)
                if "scripts" in pkg and "build" in pkg["scripts"]:
                    code, stdout, stderr = run_command("npm run build", str(app_path), timeout=120)
                    if code != 0:
                        return False, f"npm run build failed: {stderr[:200]}"
        except:
            pass
        return True, "Build successful (Node.js)"

    elif check_file_exists(app_path, "requirements.txt"):
        # Python project
        code, stdout, stderr = run_command("pip install -r requirements.txt", str(app_path), timeout=120)
        if code != 0:
            return False, f"pip install failed: {stderr[:200]}"
        return True, "Build successful (Python)"

    elif check_file_exists(app_path, "Dockerfile"):
        # Try Docker build
        code, stdout, stderr = run_command(f"docker build -t {app_name} .", str(app_path), timeout=300)
        if code != 0:
            return False, f"Docker build failed: {stderr[:200]}"
        return True, "Build successful (Docker)"

    return False, "No build system detected"


def evaluate_runtime_success(app_path: Path, app_name: str) -> tuple[bool, str]:
    """Metric 2: RUNTIME SUCCESS (Binary)"""
    # Check for different runtime systems
    if check_file_exists(app_path, "package.json"):
        try:
            with open(app_path / "package.json") as f:
                pkg = json.load(f)
                if "scripts" in pkg and "start" in pkg["scripts"]:
                    # Start and check for 5 seconds
                    proc = subprocess.Popen(
                        "npm start",
                        shell=True,
                        cwd=str(app_path),
                        stdout=subprocess.PIPE,
                        stderr=subprocess.PIPE
                    )
                    time.sleep(5)

                    if proc.poll() is None:
                        # Still running
                        proc.terminate()
                        proc.wait(timeout=5)
                        return True, "App started successfully (Node.js)"
                    else:
                        _, stderr = proc.communicate()
                        return False, f"App crashed immediately: {stderr.decode()[:200]}"
        except Exception as e:
            return False, f"Failed to start: {str(e)}"

    elif check_file_exists(app_path, "app.py") or check_file_exists(app_path, "main.py"):
        # Python Streamlit app
        app_file = "app.py" if check_file_exists(app_path, "app.py") else "main.py"
        proc = subprocess.Popen(
            f"streamlit run {app_file} --server.headless true",
            shell=True,
            cwd=str(app_path),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE
        )
        time.sleep(5)

        if proc.poll() is None:
            proc.terminate()
            proc.wait(timeout=5)
            return True, "App started successfully (Streamlit)"
        else:
            _, stderr = proc.communicate()
            return False, f"App crashed immediately: {stderr.decode()[:200]}"

    return False, "No run method detected"


def evaluate_type_safety(app_path: Path) -> tuple[Optional[bool], str]:
    """Metric 3: TYPE SAFETY (Binary)"""
    if check_file_exists(app_path, "tsconfig.json"):
        code, stdout, stderr = run_command("npx tsc --noEmit", str(app_path), timeout=60)
        if code == 0:
            return True, "TypeScript type check passed"
        else:
            return False, f"TypeScript errors: {stderr[:200]}"

    # Check for Python type checking
    if check_file_exists(app_path, "requirements.txt"):
        # Check if mypy is available
        code, _, _ = run_command("mypy --version", str(app_path))
        if code == 0:
            code, stdout, stderr = run_command("mypy .", str(app_path), timeout=60)
            if code == 0:
                return True, "mypy type check passed"
            else:
                return False, f"mypy errors: {stderr[:200]}"

    return None, "No type checking configured"


def evaluate_tests_pass(app_path: Path) -> tuple[Optional[bool], Optional[float], str]:
    """Metric 4: TESTS PASS (Binary + Coverage %)"""
    if check_file_exists(app_path, "package.json"):
        try:
            with open(app_path / "package.json") as f:
                pkg = json.load(f)
                if "scripts" in pkg and "test" in pkg["scripts"]:
                    code, stdout, stderr = run_command("npm test", str(app_path), timeout=120)
                    if code == 0:
                        # Try to extract coverage if present
                        return True, None, "Tests passed (Node.js)"
                    else:
                        return False, None, f"Tests failed: {stderr[:200]}"
        except:
            pass

    # Check for Python tests
    if check_file_exists(app_path, "test_") or (app_path / "tests").exists():
        code, stdout, stderr = run_command("pytest", str(app_path), timeout=120)
        if code == 0:
            return True, None, "Tests passed (pytest)"
        else:
            return False, None, f"Tests failed: {stderr[:200]}"

    return None, None, "No tests configured"


def evaluate_databricks_connectivity(app_path: Path) -> tuple[bool, str]:
    """Metric 5: DATABRICKS CONNECTIVITY (Binary)"""
    # Check for Databricks imports/usage
    for py_file in app_path.glob("**/*.py"):
        try:
            with open(py_file) as f:
                content = f.read()
                if "databricks" in content.lower() or "databricks-sql-connector" in content:
                    # Check if environment variables are used
                    if "DATABRICKS_HOST" in content or "DATABRICKS_TOKEN" in content:
                        return True, "Databricks connectivity detected with env vars"
                    else:
                        return False, "Databricks used but missing env var configuration"
        except:
            continue

    for ts_file in app_path.glob("**/*.ts"):
        try:
            with open(ts_file) as f:
                content = f.read()
                if "databricks" in content.lower():
                    if "DATABRICKS_HOST" in content or "DATABRICKS_TOKEN" in content:
                        return True, "Databricks connectivity detected with env vars"
                    else:
                        return False, "Databricks used but missing env var configuration"
        except:
            continue

    return False, "No Databricks connectivity detected"


def evaluate_local_runability(app_path: Path) -> tuple[int, List[str]]:
    """Metric 8: LOCAL RUNABILITY (Score 0-5)"""
    score = 0
    details = []

    # README (1 point)
    if check_file_exists(app_path, "README.md"):
        score += 1
        details.append("✓ Has README.md")
    else:
        details.append("✗ Missing README.md")

    # .env.example (1 point)
    if check_file_exists(app_path, ".env.example"):
        score += 1
        details.append("✓ Has .env.example")
    else:
        details.append("✗ Missing .env.example")

    # Install works (1 point)
    if check_file_exists(app_path, "package.json"):
        code, _, _ = run_command("npm install", str(app_path), timeout=120)
        if code == 0:
            score += 1
            details.append("✓ Install works (npm)")
        else:
            details.append("✗ Install failed (npm)")
    elif check_file_exists(app_path, "requirements.txt"):
        code, _, _ = run_command("pip install -r requirements.txt", str(app_path), timeout=120)
        if code == 0:
            score += 1
            details.append("✓ Install works (pip)")
        else:
            details.append("✗ Install failed (pip)")

    # Run command exists (1 point)
    has_run = False
    if check_file_exists(app_path, "package.json"):
        try:
            with open(app_path / "package.json") as f:
                pkg = json.load(f)
                if "scripts" in pkg and ("start" in pkg["scripts"] or "dev" in pkg["scripts"]):
                    score += 1
                    details.append("✓ Has run command")
                    has_run = True
        except:
            pass
    elif check_file_exists(app_path, "app.py") or check_file_exists(app_path, "main.py"):
        score += 1
        details.append("✓ Has run command (Streamlit)")
        has_run = True

    if not has_run:
        details.append("✗ No run command")

    # Starts successfully (1 point) - already evaluated in runtime_success
    # We'll leave this for now
    details.append("○ Start success evaluated separately")

    return score, details


def evaluate_deployability(app_path: Path) -> tuple[int, List[str]]:
    """Metric 9: DEPLOYABILITY (Score 0-5)"""
    score = 0
    details = []

    # Dockerfile (1 point)
    if check_file_exists(app_path, "Dockerfile"):
        score += 1
        details.append("✓ Has Dockerfile")

        # Multi-stage build (1 point)
        try:
            with open(app_path / "Dockerfile") as f:
                content = f.read()
                if "FROM" in content and content.count("FROM") > 1:
                    score += 1
                    details.append("✓ Multi-stage build")
                else:
                    details.append("✗ Not multi-stage build")

                # Health check (1 point)
                if "HEALTHCHECK" in content:
                    score += 1
                    details.append("✓ Has health check")
                else:
                    details.append("✗ No health check")

                # No hardcoded secrets (1 point)
                suspicious = ["password=", "token=", "secret=", "api_key="]
                has_hardcoded = any(s in content.lower() for s in suspicious)
                if not has_hardcoded:
                    score += 1
                    details.append("✓ No hardcoded secrets")
                else:
                    details.append("✗ Possible hardcoded secrets")
        except:
            details.append("✗ Could not read Dockerfile")
    else:
        details.append("✗ Missing Dockerfile")
        details.append("✗ No multi-stage build")
        details.append("✗ No health check")
        details.append("✗ No hardcoded secrets check")

    # app.yaml (1 point)
    if check_file_exists(app_path, "app.yaml"):
        score += 1
        details.append("✓ Has app.yaml")
    else:
        details.append("✗ Missing app.yaml")

    return score, details


def evaluate_app(app_name: str) -> Dict[str, Any]:
    """Evaluate a single app using all 9 metrics."""
    print(f"\n{'='*60}")
    print(f"Evaluating: {app_name}")
    print(f"{'='*60}")

    app_path = APP_DIR / app_name
    result = {
        "app_name": app_name,
        "metrics": {},
        "issues": []
    }

    # Metric 1: Build Success
    print("1. Checking build success...")
    build_success, build_msg = evaluate_build_success(app_path, app_name)
    result["metrics"]["build_success"] = build_success
    result["issues"].append(f"Build: {build_msg}")
    print(f"   {'✓' if build_success else '✗'} {build_msg}")

    # Metric 2: Runtime Success
    print("2. Checking runtime success...")
    runtime_success, runtime_msg = evaluate_runtime_success(app_path, app_name)
    result["metrics"]["runtime_success"] = runtime_success
    result["issues"].append(f"Runtime: {runtime_msg}")
    print(f"   {'✓' if runtime_success else '✗'} {runtime_msg}")

    # Metric 3: Type Safety
    print("3. Checking type safety...")
    type_safety, type_msg = evaluate_type_safety(app_path)
    result["metrics"]["type_safety"] = type_safety
    result["issues"].append(f"Type Safety: {type_msg}")
    print(f"   {('✓' if type_safety else '✗') if type_safety is not None else 'N/A'} {type_msg}")

    # Metric 4: Tests Pass
    print("4. Checking tests...")
    tests_pass, coverage, tests_msg = evaluate_tests_pass(app_path)
    result["metrics"]["tests_pass"] = tests_pass
    result["metrics"]["test_coverage"] = coverage
    result["issues"].append(f"Tests: {tests_msg}")
    print(f"   {('✓' if tests_pass else '✗') if tests_pass is not None else 'N/A'} {tests_msg}")

    # Metric 5: Databricks Connectivity
    print("5. Checking Databricks connectivity...")
    db_conn, db_msg = evaluate_databricks_connectivity(app_path)
    result["metrics"]["databricks_connectivity"] = db_conn
    result["issues"].append(f"Databricks: {db_msg}")
    print(f"   {'✓' if db_conn else '✗'} {db_msg}")

    # Metric 6: Data Returned (Not implemented)
    result["metrics"]["data_returned"] = None
    print("6. Data returned: Not implemented")

    # Metric 7: UI Renders (Not implemented)
    result["metrics"]["ui_renders"] = None
    print("7. UI renders: Not implemented")

    # Metric 8: Local Runability
    print("8. Checking local runability...")
    runability_score, runability_details = evaluate_local_runability(app_path)
    result["metrics"]["local_runability_score"] = runability_score
    result["metrics"]["local_runability_details"] = runability_details
    print(f"   Score: {runability_score}/5")
    for detail in runability_details:
        print(f"      {detail}")

    # Metric 9: Deployability
    print("9. Checking deployability...")
    deploy_score, deploy_details = evaluate_deployability(app_path)
    result["metrics"]["deployability_score"] = deploy_score
    result["metrics"]["deployability_details"] = deploy_details
    print(f"   Score: {deploy_score}/5")
    for detail in deploy_details:
        print(f"      {detail}")

    return result


def generate_summary(results: List[Dict[str, Any]]) -> Dict[str, Any]:
    """Generate summary statistics."""
    total_apps = len(results)

    # Count successes
    build_success_count = sum(1 for r in results if r["metrics"].get("build_success"))
    runtime_success_count = sum(1 for r in results if r["metrics"].get("runtime_success"))
    type_safety_count = sum(1 for r in results if r["metrics"].get("type_safety") is True)
    type_safety_na_count = sum(1 for r in results if r["metrics"].get("type_safety") is None)
    tests_pass_count = sum(1 for r in results if r["metrics"].get("tests_pass") is True)
    tests_na_count = sum(1 for r in results if r["metrics"].get("tests_pass") is None)
    databricks_count = sum(1 for r in results if r["metrics"].get("databricks_connectivity"))

    # Average scores
    avg_runability = sum(r["metrics"].get("local_runability_score", 0) for r in results) / total_apps
    avg_deployability = sum(r["metrics"].get("deployability_score", 0) for r in results) / total_apps

    return {
        "total_apps": total_apps,
        "metrics_summary": {
            "build_success": f"{build_success_count}/{total_apps}",
            "runtime_success": f"{runtime_success_count}/{total_apps}",
            "type_safety": f"{type_safety_count}/{total_apps - type_safety_na_count} (N/A: {type_safety_na_count})",
            "tests_pass": f"{tests_pass_count}/{total_apps - tests_na_count} (N/A: {tests_na_count})",
            "databricks_connectivity": f"{databricks_count}/{total_apps}",
            "data_returned": "Not implemented",
            "ui_renders": "Not implemented",
            "local_runability_avg": f"{avg_runability:.2f}/5",
            "deployability_avg": f"{avg_deployability:.2f}/5"
        }
    }


def generate_markdown_report(summary: Dict[str, Any], results: List[Dict[str, Any]]) -> str:
    """Generate markdown report."""
    md = "# Evaluation Report\n\n"
    md += f"**Generated:** {time.strftime('%Y-%m-%d %H:%M:%S')}\n\n"

    md += "## Summary\n\n"
    md += f"- **Total Apps Evaluated:** {summary['total_apps']}\n\n"

    md += "### Metrics Overview\n\n"
    for metric, value in summary["metrics_summary"].items():
        md += f"- **{metric.replace('_', ' ').title()}:** {value}\n"

    md += "\n## Individual App Results\n\n"

    for result in results:
        app_name = result["app_name"]
        metrics = result["metrics"]

        md += f"### {app_name}\n\n"

        # Binary metrics
        md += "**Binary Metrics:**\n"
        md += f"- Build Success: {'✓' if metrics.get('build_success') else '✗'}\n"
        md += f"- Runtime Success: {'✓' if metrics.get('runtime_success') else '✗'}\n"

        type_safety = metrics.get('type_safety')
        md += f"- Type Safety: {('✓' if type_safety else '✗') if type_safety is not None else 'N/A'}\n"

        tests_pass = metrics.get('tests_pass')
        md += f"- Tests Pass: {('✓' if tests_pass else '✗') if tests_pass is not None else 'N/A'}\n"

        md += f"- Databricks Connectivity: {'✓' if metrics.get('databricks_connectivity') else '✗'}\n"
        md += f"- Data Returned: N/A (not implemented)\n"
        md += f"- UI Renders: N/A (not implemented)\n\n"

        # Scored metrics
        md += "**Scored Metrics:**\n"
        md += f"- Local Runability: {metrics.get('local_runability_score', 0)}/5\n"
        for detail in metrics.get('local_runability_details', []):
            md += f"  - {detail}\n"

        md += f"- Deployability: {metrics.get('deployability_score', 0)}/5\n"
        for detail in metrics.get('deployability_details', []):
            md += f"  - {detail}\n"

        md += "\n**Issues:**\n"
        for issue in result["issues"]:
            md += f"- {issue}\n"

        md += "\n---\n\n"

    return md


def main():
    """Main evaluation function."""
    # Get list of apps
    apps = [d for d in os.listdir(APP_DIR) if (APP_DIR / d).is_dir()]
    apps = sorted(apps)

    print(f"Found {len(apps)} apps to evaluate")

    # Evaluate each app
    results = []
    for app_name in apps:
        try:
            result = evaluate_app(app_name)
            results.append(result)
        except Exception as e:
            print(f"ERROR evaluating {app_name}: {e}")
            results.append({
                "app_name": app_name,
                "metrics": {},
                "issues": [f"Evaluation error: {str(e)}"]
            })

    # Generate summary
    summary = generate_summary(results)

    # Save JSON report
    report = {
        "summary": summary,
        "apps": results
    }

    with open("evaluation_report.json", "w") as f:
        json.dump(report, f, indent=2)
    print("\n✓ Saved evaluation_report.json")

    # Save Markdown report
    md_report = generate_markdown_report(summary, results)
    with open("EVALUATION_REPORT.md", "w") as f:
        f.write(md_report)
    print("✓ Saved EVALUATION_REPORT.md")

    print(f"\nEvaluation complete! Evaluated {len(results)} apps.")


if __name__ == "__main__":
    main()
