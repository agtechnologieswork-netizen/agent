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
    "create a single user todo app with in memory storage",
    "make a simple +1 counter web app",
    "build a tool that converts sq feet to square meters, and vice versa",
]


def run_single_generation(prompt: str, wipe_db: bool = False) -> RunResult:
    codegen = AppBuilder(wipe_db=wipe_db, suppress_logs=True)
    metrics = codegen.run(prompt, wipe_db=wipe_db)
    return {
        "prompt": prompt,
        "success": True,
        "metrics": metrics,
        "error": None,
    }


def main(wipe_db: bool = False, n_jobs: int = -1) -> None:
    print(f"Starting bulk generation for {len(PROMPTS)} prompts...")
    print(f"Parallel jobs: {n_jobs}")
    print(f"Wipe DB: {wipe_db}\n")

    results = Parallel(n_jobs=n_jobs)(
        delayed(run_single_generation)(prompt, wipe_db) for prompt in tqdm(PROMPTS, desc="Generating apps")
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
