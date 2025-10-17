"""Bulk runner for generating multiple apps from hardcoded prompts."""

import json
import subprocess
from datetime import datetime
from pathlib import Path
from typing import TypedDict

from joblib import Parallel, delayed

from codegen import AppBuilder, GenerationMetrics


class RunResult(TypedDict):
    prompt: str
    success: bool
    metrics: GenerationMetrics | None
    error: str | None
    app_dir: str | None
    screenshot_path: str | None
    screenshot_log: str | None


PROMPTS = [
    "Build a churn risk dashboard showing customers with less than 30 day login activity, declining usage trends, and support ticket volume. Calculate a risk score.",
    "Show daily revenue by channel (store/web/catalog) for the last 90 days with week-over-week growth rates and contribution percentages.",
    "Create customer segments using RFM analysis (recency, frequency, monetary). Show 4-5 clusters with average spend, purchase frequency, and last order date.",
    "Build a taxi fare prediction model using trip distance, time of day, and pickup location. Show actual vs predicted fares with error metrics.",
    "Identify slow-moving inventory: products with more than 90 days in stock, low turnover ratio, and current warehouse capacity by location.",
    "Create a 360-degree customer view: lifetime orders, total spent, average order value, preferred categories, and payment methods used.",
    "Build a market basket analysis showing top 10 product pairs frequently purchased together. Include bundle revenue opportunity.",
    "Forecast next quarter revenue using historical sales with seasonal decomposition. Show trend, seasonality, and confidence intervals.",
    "Monitor ML feature drift: compare training vs production distributions for top features. Flag significant distribution changes.",
    "Compare conversion rates and average order value across store/web/catalog channels. Break down by customer segment.",
    "Correlate support ticket categories with churn within 30 days. Show relationship strength and ticket volume by issue type.",
    "Analyze price elasticity by category: show revenue impact of different price changes. Identify optimal price points.",
    "Build supplier scorecard: on-time delivery percentage, defect rate, average lead time, and fill rate. Rank top 10 suppliers.",
    "Map sales density by zip code with heatmap visualization. Show top 20 zips by revenue and compare to population density.",
    "Calculate CAC by marketing channel (paid search, social, email, organic). Show CAC to LTV ratio and payback period in months.",
    "Predict subscription tier changes: customers likely to upgrade (high usage near limit) or downgrade (consistently low usage for 60 days).",
    "Show product profitability: revenue minus returns percentage minus discount cost. Rank bottom 20 products by net margin.",
    "Build warehouse efficiency dashboard: orders per hour, fulfillment SLA (percentage shipped within 24 hours), and capacity utilization by facility.",
    "Calculate customer LTV by acquisition cohort: average revenue per customer at 12, 24, 36 months. Show retention curves.",
    "Measure promotion ROI: incremental revenue during promo vs cost, with 7-day post-promotion lift. Flag underperforming promotions.",
]


def capture_screenshot(app_dir: str) -> tuple[str | None, str]:
    """Capture screenshot for generated app using Dagger.

    Runs dagger from the template directory (sidecar pattern) and passes
    the generated app path as the source parameter.

    Returns:
        Tuple of (screenshot_path, log_output):
        - screenshot_path: Path to screenshot.png if successful, None otherwise
        - log_output: Full stdout/stderr from the dagger command execution
    """
    app_path = Path(app_dir).resolve()
    template_path = Path(__file__).parent.parent.parent / "dataapps" / "template_trpc"
    screenshot_dest = app_path / "screenshot.png"

    try:
        result = subprocess.run(
            ["dagger", "call", "screenshot", f"--source={app_path}", "export", f"--path={screenshot_dest}"],
            cwd=str(template_path),
            capture_output=True,
            text=True,
            timeout=300,  # 5 minute timeout for dagger operations
        )

        log = f"=== STDOUT ===\n{result.stdout}\n\n=== STDERR ===\n{result.stderr}\n\n=== EXIT CODE ===\n{result.returncode}"

        if result.returncode == 0 and screenshot_dest.exists():
            return str(screenshot_dest), log
        else:
            return None, log

    except subprocess.TimeoutExpired:
        log = "Screenshot capture timed out after 5 minutes"
        return None, log
    except Exception as e:
        log = f"Exception during screenshot capture: {type(e).__name__}: {str(e)}"
        return None, log


def run_single_generation(prompt: str, wipe_db: bool = False, use_subagents: bool = False) -> RunResult:
    codegen = AppBuilder(wipe_db=wipe_db, suppress_logs=True, use_subagents=use_subagents)
    metrics = codegen.run(prompt, wipe_db=wipe_db)
    app_dir = metrics.get("app_dir") if metrics else None

    # capture screenshot if app was generated successfully
    screenshot_path = None
    screenshot_log = None
    if app_dir:
        screenshot_path, screenshot_log = capture_screenshot(app_dir)

    return {
        "prompt": prompt,
        "success": True,
        "metrics": metrics,
        "error": None,
        "app_dir": app_dir,
        "screenshot_path": screenshot_path,
        "screenshot_log": screenshot_log,
    }


def main(wipe_db: bool = False, n_jobs: int = -1, use_subagents: bool = False) -> None:
    print(f"Starting bulk generation for {len(PROMPTS)} prompts...")
    print(f"Parallel jobs: {n_jobs}")
    print(f"Wipe DB: {wipe_db}")
    print(f"Use subagents: {use_subagents}\n")

    results: list[RunResult] = Parallel(n_jobs=n_jobs, verbose=10)(  # type: ignore[assignment]
        delayed(run_single_generation)(prompt, wipe_db, use_subagents) for prompt in PROMPTS
    )

    successful: list[RunResult] = []
    failed: list[RunResult] = []
    for r in results:
        success = r["success"]
        if success:
            successful.append(r)
        else:
            failed.append(r)

    successful_with_metrics: list[RunResult] = []
    for r in successful:
        metrics = r["metrics"]
        if metrics is not None:
            successful_with_metrics.append(r)

    total_cost = 0.0
    total_input_tokens = 0
    total_output_tokens = 0
    total_turns = 0
    for r in successful_with_metrics:
        metrics = r["metrics"]
        assert metrics is not None
        total_cost += metrics["cost_usd"]
        total_input_tokens += metrics["input_tokens"]
        total_output_tokens += metrics["output_tokens"]
        total_turns += metrics["turns"]
    # calculate screenshot statistics
    screenshot_successful = 0
    screenshot_failed = 0
    for r in successful:
        if r["screenshot_path"] is not None:
            screenshot_successful += 1
        elif r["app_dir"] is not None:  # only count as failed if app was generated but screenshot failed
            screenshot_failed += 1

    print(f"\n{'=' * 80}")
    print(f"Bulk Generation Summary")
    print(f"{'=' * 80}")
    print(f"Total prompts: {len(PROMPTS)}")
    print(f"Successful: {len(successful)}")
    print(f"Failed: {len(failed)}")
    print(f"\nScreenshots captured: {screenshot_successful}")
    print(f"Screenshot failures: {screenshot_failed}")
    if screenshot_failed > 0:
        print(f"  (Screenshot logs available in JSON output)")
    print(f"\nTotal cost: ${total_cost:.4f}")
    print(f"Total input tokens: {total_input_tokens}")
    print(f"Total output tokens: {total_output_tokens}")
    print(f"Total turns: {total_turns}")

    if successful_with_metrics:
        avg_cost = total_cost / len(successful_with_metrics)
        avg_input = total_input_tokens / len(successful_with_metrics)
        avg_output = total_output_tokens / len(successful_with_metrics)
        avg_turns = total_turns / len(successful_with_metrics)
        print(f"\nAverage per generation:")
        print(f"  Cost: ${avg_cost:.4f}")
        print(f"  Input tokens: {avg_input:.0f}")
        print(f"  Output tokens: {avg_output:.0f}")
        print(f"  Turns: {avg_turns:.1f}")

    if len(failed) > 0:
        print(f"\n{'=' * 80}")
        print(f"Failed generations:")
        print(f"{'=' * 80}")
        for r in failed:
            prompt = r["prompt"]
            error = r["error"]
            print(f"  - {prompt[:50]}...")
            if error is not None:
                print(f"    Error: {error}")

    if len(successful) > 0:
        apps_with_dirs: list[tuple[str, str]] = []
        for r in successful:
            prompt = r["prompt"]
            app_dir = r["app_dir"]
            if app_dir is not None:
                apps_with_dirs.append((prompt, app_dir))

        if apps_with_dirs:
            print(f"\n{'=' * 80}")
            print(f"Generated apps:")
            print(f"{'=' * 80}")
            for prompt, app_dir in apps_with_dirs:
                print(f"  - {prompt[:60]}...")
                print(f"    Dir: {app_dir}")

    print(f"\n{'=' * 80}\n")

    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    output_file = Path(f"bulk_run_results_{timestamp}.json")

    output_file.write_text(json.dumps(results, indent=2))
    print(f"Results saved to {output_file}")


if __name__ == "__main__":
    import fire

    fire.Fire(main)
