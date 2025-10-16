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
    "Build me a dashboard to identify customers who might cancel soon. I think we have customer info and their usage metrics somewhere in the churn or customer schemas.",
    "I need to see our sales trends across all channels - stores, web, catalog. The data should be in something like sales or revenue tables.",
    "Can you create a customer clustering view? We have demographics and purchase history somewhere, probably in customer-related tables.",
    "Make a taxi trip analysis dashboard with fare predictions and route patterns. The trip data should be in some taxi or transportation schema.",
    "I want to track which products are sitting in our warehouses too long. Check inventory or warehouse tables for stock levels and movement.",
    "Build a complete customer profile view showing everything about a customer - their orders, preferences, payments. Look in customer and sales schemas.",
    "Create something that shows which products customers buy together. The purchase data is probably in sales or transactions somewhere.",
    "I need revenue forecasting with seasonal patterns. Pull historical sales from wherever we keep store or web sales data.",
    "Build a monitoring system for our ML features - are they drifting? The feature data should be in ML or feature store schemas.",
    "Compare sales performance across our different channels. Data might be split across store, web, and catalog tables.",
    "Show me how customer support calls relate to cancellations. Look for support call logs and customer status data.",
    "Analyze which price points work best for each product category. Check sales and product tables for pricing and revenue.",
    "I need a supplier scorecard - delivery times, quality, fulfillment rates. The data is probably in supplier or order tables.",
    "Create a geographic breakdown of sales by region or zip code. Sales and customer address data should have this.",
    "Calculate our customer acquisition costs by marketing channel. Look at customer sign-ups and promotional data.",
    "Predict which customers will upgrade or downgrade their contracts. We have customer tiers and usage somewhere.",
    "Show me which products are actually profitable after returns and discounts. Check sales and returns tables.",
    "Build a warehouse operations dashboard with fulfillment speed and capacity. Data is in warehouse or inventory schemas.",
    "Calculate lifetime value for each customer segment. Pull all their historical purchases from sales tables.",
    "Measure how effective our promotions are. Look at promotion tables and compare sales during and after campaigns.",
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
