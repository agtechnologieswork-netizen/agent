"""Bulk runner for generating multiple apps from hardcoded prompts."""

import json
from datetime import datetime
from pathlib import Path
from typing import TypedDict

from joblib import Parallel, delayed
from tqdm import tqdm

from codegen import AppBuilder, GenerationMetrics


class RunResult(TypedDict):
    prompt: str
    success: bool
    metrics: GenerationMetrics | None
    error: str | None


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


def run_single_generation(prompt: str, wipe_db: bool = False, use_subagents: bool = False) -> RunResult:
    codegen = AppBuilder(wipe_db=wipe_db, suppress_logs=True, use_subagents=use_subagents)
    metrics = codegen.run(prompt, wipe_db=wipe_db)
    return {
        "prompt": prompt,
        "success": True,
        "metrics": metrics,
        "error": None,
    }


def main(wipe_db: bool = False, n_jobs: int = -1, use_subagents: bool = False) -> None:
    print(f"Starting bulk generation for {len(PROMPTS)} prompts...")
    print(f"Parallel jobs: {n_jobs}")
    print(f"Wipe DB: {wipe_db}")
    print(f"Use subagents: {use_subagents}\n")

    with tqdm(total=len(PROMPTS), desc="Generating apps") as pbar:
        def update_progress(result: RunResult) -> RunResult:
            pbar.update(1)
            return result

        results = Parallel(n_jobs=n_jobs)(
            delayed(lambda p: update_progress(run_single_generation(p, wipe_db, use_subagents)))(prompt)
            for prompt in PROMPTS
        )

    successful = [r for r in results if r["success"]]
    failed = [r for r in results if not r["success"]]

    total_cost = sum(r["metrics"]["cost_usd"] for r in successful if r["metrics"])
    total_input_tokens = sum(r["metrics"]["input_tokens"] for r in successful if r["metrics"])
    total_output_tokens = sum(r["metrics"]["output_tokens"] for r in successful if r["metrics"])
    total_turns = sum(r["metrics"]["turns"] for r in successful if r["metrics"])
    print(f"\n{'=' * 80}")
    print(f"Bulk Generation Summary")
    print(f"{'=' * 80}")
    print(f"Total prompts: {len(PROMPTS)}")
    print(f"Successful: {len(successful)}")
    print(f"Failed: {len(failed)}")
    print(f"\nTotal cost: ${total_cost:.4f}")
    print(f"Total input tokens: {total_input_tokens}")
    print(f"Total output tokens: {total_output_tokens}")
    print(f"Total turns: {total_turns}")

    if successful:
        avg_cost = total_cost / len(successful)
        avg_input = total_input_tokens / len(successful)
        avg_output = total_output_tokens / len(successful)
        avg_turns = total_turns / len(successful)
        print(f"\nAverage per generation:")
        print(f"  Cost: ${avg_cost:.4f}")
        print(f"  Input tokens: {avg_input:.0f}")
        print(f"  Output tokens: {avg_output:.0f}")
        print(f"  Turns: {avg_turns:.1f}")

    if failed:
        print(f"\n{'=' * 80}")
        print(f"Failed generations:")
        print(f"{'=' * 80}")
        for r in failed:
            print(f"  - {r['prompt'][:50]}...")
            print(f"    Error: {r['error']}")

    print(f"\n{'=' * 80}\n")

    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    output_file = Path(f"bulk_run_results_{timestamp}.json")

    output_file.write_text(json.dumps(results, indent=2))
    print(f"Results saved to {output_file}")


if __name__ == "__main__":
    import fire

    fire.Fire(main)
