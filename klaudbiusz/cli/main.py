import fire
from codegen import AppBuilder


def run(prompt: str, wipe_db: bool = True, suppress_logs: bool = False, use_subagents: bool = False):
    """Run app builder with given prompt.

    Args:
        prompt: The prompt describing what to build
        wipe_db: Whether to wipe database on start
        suppress_logs: Whether to suppress logs
        use_subagents: Whether to enable subagent delegation (e.g., dataresearch)

    Usage:
        python main.py "your prompt here" --use_subagents
        python main.py "build dashboard" --use_subagents --no-wipe_db
    """
    builder = AppBuilder(wipe_db=wipe_db, suppress_logs=suppress_logs, use_subagents=use_subagents)
    metrics = builder.run(prompt, wipe_db=wipe_db)
    print(f"\n{'=' * 80}")
    print(f"Final metrics:")
    print(f"  Cost: ${metrics['cost_usd']:.4f}")
    print(f"  Turns: {metrics['turns']}")
    print(f"  App dir: {metrics.get('app_dir', 'NOT CAPTURED')}")
    print(f"{'=' * 80}\n")
    return metrics


def main():
    fire.Fire(run)


if __name__ == "__main__":
    main()
