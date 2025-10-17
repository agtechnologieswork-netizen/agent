import fire
from datetime import datetime
from codegen import AppBuilder


def run(
    prompt: str,
    app_name: str | None = "baseline",
    wipe_db: bool = True,
    suppress_logs: bool = False,
    use_subagents: bool = False,
):
    if app_name is None:
        app_name = f"app-{datetime.now().strftime('%Y%m%d-%H%M%S')}"

    builder = AppBuilder(app_name=app_name, wipe_db=wipe_db, suppress_logs=suppress_logs, use_subagents=use_subagents)
    metrics = builder.run(prompt, wipe_db=wipe_db)
    print(f"\n{'=' * 80}")
    print("Final metrics:")
    print(f"  Cost: ${metrics['cost_usd']:.4f}")
    print(f"  Turns: {metrics['turns']}")
    print(f"  App dir: {metrics.get('app_dir', 'NOT CAPTURED')}")
    print(f"{'=' * 80}\n")
    return metrics


def main():
    fire.Fire(run)


if __name__ == "__main__":
    main()
