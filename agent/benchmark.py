#!/usr/bin/env python3
"""
LLM model benchmarking tool for agent generation.

Runs a matrix ablation study capturing:
- Generated source code
- Telemetry data (via CUMULATIVE_TELEMETRY_LOG env var)
- Success/failure status based on Docker health check

Usage:
  uv run python benchmark.py
"""

import asyncio
import subprocess
import itertools
import json
import os
import csv
import sys
import shutil
import tempfile
from pathlib import Path
from datetime import datetime
from typing import Dict, List, Tuple, Any
import fire
from tests.test_e2e import run_e2e


def log(msg: str) -> None:
    print(f"[{datetime.now().strftime('%H:%M:%S')}] {msg}")


def get_matrix_configurations() -> Tuple[Dict[str, str], List[str], Dict[str, str], Dict[str, str]]:
    """Define the matrix components for ablation study."""

    prompts = {
        "counter": "Implement a simple app with a counter of clicks on a single button with a backend with persistence in DB and a frontend. The simplest one possible!",
        "todo": "Create a todo list app with backend persistence and ability to add, delete, and mark items as done. The simplest one possible!",
        # "blog": "Build a blog application with posts, comments, and user authentication"
    }

    template_ids = ["trpc_agent", "nicegui_agent"]

    coding_models = {
        "claude": "anthropic:claude-sonnet-4-20250514",
        "qwen3-480b-35a": "openrouter:qwen/qwen3-coder"
    }

    universal_models = {
        "gemini": "gemini:gemini-2.5-flash-preview-05-20",
    }

    return prompts, template_ids, coding_models, universal_models


class GenerationCapture:
    """Helper class to capture generation artifacts."""

    def __init__(self, output_dir: str):
        self.output_dir = Path(output_dir)
        self.captured_temp_dir = None
        self.success = False

    async def run_with_capture(self, prompt: str, template_id: str) -> bool:
        """Run generation and capture all artifacts."""
        try:
            # Run the generation with standalone=False to ensure Docker health check
            await run_e2e(
                prompt=prompt,
                standalone=False,  # ensures Docker validation
                with_edit=False,
                template_id=template_id
            )

            self.success = True
            log("Generation completed successfully")
            return True

        except Exception as e:
            log(f"Generation failed: {e}")
            self.success = False
            return False


async def run_single_generation(prompt: str, template_id: str, output_dir: str) -> None:
    """
    Run a single generation and save all artifacts.

    Args:
        prompt: The prompt to generate from
        template_id: Template ID (trpc_agent, nicegui_agent, laravel_agent)
        output_dir: Directory to save all artifacts
    """
    output_path = Path(output_dir)
    output_path.mkdir(parents=True, exist_ok=True)

    log("Starting generation:")
    log(f"  Prompt: {prompt[:50]}...")
    log(f"  Template: {template_id}")
    log(f"  Output: {output_dir}")

    # Initialize capture helper
    capture = GenerationCapture(output_dir)

    # We need to monkey patch run_e2e to capture the temp_dir
    # The challenge is that run_e2e creates its own tempfile.TemporaryDirectory
    # We'll patch tempfile.TemporaryDirectory to capture the path
    original_tempdir = tempfile.TemporaryDirectory
    captured_dirs = []

    class CapturingTempDir(original_tempdir):
        def __init__(self, *args, **kwargs):
            super().__init__(*args, **kwargs)
            captured_dirs.append(self.name)

        def __enter__(self):
            result = super().__enter__()
            if len(captured_dirs) > 0:
                capture.captured_temp_dir = captured_dirs[-1]
            return result

        def __exit__(self, exc_type, exc_val, exc_tb):
            # Copy contents before the original __exit__ deletes the directory
            if (capture.captured_temp_dir and
                Path(capture.captured_temp_dir).exists() and
                capture.captured_temp_dir == self.name):
                try:
                    source_dir = output_path / "source_code"
                    if source_dir.exists():
                        shutil.rmtree(source_dir)

                    # Copy the entire generated project before cleanup
                    shutil.copytree(self.name, source_dir)
                    log(f"Source code saved to {source_dir}")

                    # List what was generated for debugging
                    generated_files = list(source_dir.rglob("*"))
                    log(f"Generated {len(generated_files)} files/directories")
                except Exception as e:
                    log(f"Failed to copy temp directory: {e}")

            # Now let the original cleanup happen
            return super().__exit__(exc_type, exc_val, exc_tb)

    # Apply monkey patch
    tempfile.TemporaryDirectory = CapturingTempDir

    try:
        # Run the generation
        success = await capture.run_with_capture(prompt, template_id)

        # Copying happens automatically in CapturingTempDir.__exit__

        # Exit with appropriate code
        sys.exit(0 if success else 1)

    except Exception as e:
        print(f"Fatal error in generation: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(2)

    finally:
        # Restore original tempfile
        tempfile.TemporaryDirectory = original_tempdir


def save_run_results(run_dir: Path, subprocess_result: subprocess.CompletedProcess,
                     env_vars: Dict[str, str], duration: float, config_info: Dict[str, Any]) -> None:
    """Save all run artifacts and results."""

    # Determine success based on exit code (run_e2e raises exception if Docker unhealthy)
    success = subprocess_result.returncode == 0
    docker_healthy = success  # If exit code is 0, Docker was healthy

    status = {
        "success": success,
        "exit_code": subprocess_result.returncode,
        "docker_healthy": docker_healthy,
        "duration_seconds": duration,
        "timestamp": datetime.now().isoformat(),
        "config": {
            "prompt_name": config_info["prompt_name"],
            "template_id": config_info["template_id"],
            "coding_model_name": config_info["coding_model_name"],
            "universal_model_name": config_info["universal_model_name"],
            "LLM_BEST_CODING_MODEL": env_vars.get("LLM_BEST_CODING_MODEL"),
            "LLM_UNIVERSAL_MODEL": env_vars.get("LLM_UNIVERSAL_MODEL"),
            "CUMULATIVE_TELEMETRY_LOG": env_vars.get("CUMULATIVE_TELEMETRY_LOG")
        }
    }

    # Save all artifacts
    (run_dir / "status.json").write_text(json.dumps(status, indent=2))
    (run_dir / "stdout.log").write_text(subprocess_result.stdout)
    (run_dir / "stderr.log").write_text(subprocess_result.stderr)

    log(f"  Result: {'✓ SUCCESS' if success else '✗ FAILED'}")
    if not success:
        log(f"  Error: Exit code {subprocess_result.returncode}, Docker healthy: {docker_healthy}")


def generate_summary(results_dir: Path = Path("benchmark_results")) -> None:
    """Generate CSV summary of all runs."""
    results = []

    for run_dir in results_dir.iterdir():
        if not run_dir.is_dir():
            continue

        status_file = run_dir / "status.json"
        if not status_file.exists():
            continue

        # Load status
        status = json.loads(status_file.read_text())

        # Load telemetry if exists
        telemetry_file = run_dir / "telemetry.json"
        total_tokens = 0
        total_calls = 0
        if telemetry_file.exists():
            telemetry = json.loads(telemetry_file.read_text())
            for model_stats in telemetry.values():
                total_tokens += model_stats.get("total_input_tokens", 0) + model_stats.get("total_output_tokens", 0)
                total_calls += model_stats.get("total_calls", 0)

        config = status.get("config", {})
        results.append({
            "run_name": run_dir.name,
            "prompt_name": config.get("prompt_name"),
            "template_id": config.get("template_id"),
            "coding_model": config.get("coding_model_name"),
            "universal_model": config.get("universal_model_name"),
            "success": status["success"],
            "docker_healthy": status["docker_healthy"],
            "duration_seconds": status["duration_seconds"],
            "total_tokens": total_tokens,
            "total_model_calls": total_calls,
            "exit_code": status["exit_code"],
            "timestamp": status["timestamp"]
        })

    if not results:
        print("No results found to summarize")
        return

    # Save as CSV
    summary_file = results_dir / "summary.csv"
    with open(summary_file, "w", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=results[0].keys())
        writer.writeheader()
        writer.writerows(results)

    log(f"Summary saved to {summary_file}")

    # Print quick stats
    total_runs = len(results)
    successful_runs = sum(1 for r in results if r["success"])
    log(f"Total runs: {total_runs}")
    log(f"Successful runs: {successful_runs}")
    log(f"Success rate: {successful_runs/total_runs*100:.1f}%")


def single(prompt: str, template_id: str, output_dir: str) -> None:
    """Run a single generation."""
    asyncio.run(run_single_generation(prompt, template_id, output_dir))


def matrix() -> None:
    """Run the full matrix benchmark study."""
    resume = False
    summary_only = False
    filter_template = None
    filter_prompt = None
    timeout_minutes = 10

    if summary_only:
        generate_summary()
        return

    prompts, template_ids, coding_models, universal_models = get_matrix_configurations()

    # Apply filters if specified
    if filter_template:
        template_ids = [t for t in template_ids if t == filter_template]
    if filter_prompt:
        prompts = {k: v for k, v in prompts.items() if k == filter_prompt}

    # Generate all combinations
    matrix_combinations = list(itertools.product(
        prompts.items(),
        template_ids,
        coding_models.items(),
        universal_models.items()
    ))

    log(f"Total runs to execute: {len(matrix_combinations)}")
    if resume:
        log("Resume mode: will skip completed runs")

    results_dir = Path("benchmark_results")
    results_dir.mkdir(exist_ok=True)

    # Sequential execution (required due to Docker port 80)
    for idx, config in enumerate(matrix_combinations, 1):
        (prompt_name, prompt_text), template_id, (coding_name, coding_model), (universal_name, universal_model) = config

        # Generate readable run name
        run_name = f"{prompt_name}_{template_id.replace('_', '-')}_{coding_name}_{universal_name}"
        run_dir = results_dir / run_name

        # Skip if already completed and in resume mode
        if resume and (run_dir / "status.json").exists():
            log(f"[{idx}/{len(matrix_combinations)}] Skipping {run_name} - already completed")
            continue

        log(f"[{idx}/{len(matrix_combinations)}] Running: {run_name}")
        run_dir.mkdir(parents=True, exist_ok=True)

        # Set unique telemetry log path
        telemetry_path = run_dir / "telemetry.json"

        # Prepare environment
        env = os.environ.copy()
        env["CUMULATIVE_TELEMETRY_LOG"] = str(telemetry_path)
        env["LLM_BEST_CODING_MODEL"] = coding_model
        env["LLM_UNIVERSAL_MODEL"] = universal_model

        config_info = {
            "prompt_name": prompt_name,
            "template_id": template_id,
            "coding_model_name": coding_name,
            "universal_model_name": universal_name
        }

        # Run generation subprocess
        start_time = datetime.now()
        try:
            result = subprocess.run(
                ["uv", "run", "python", "benchmark.py", "single",
                 "--prompt", prompt_text,
                 "--template-id", template_id,
                 "--output-dir", str(run_dir)],
                env=env,
                capture_output=True,
                text=True,
                timeout=timeout_minutes * 60
            )
        except subprocess.TimeoutExpired as e:
            log(f"  TIMEOUT after {timeout_minutes} minutes")
            # Capture whatever output we got before timeout
            stdout = e.stdout.decode() if e.stdout else ""
            stderr = e.stderr.decode() if e.stderr else ""
            result = subprocess.CompletedProcess(
                args=e.args, returncode=124,
                stdout=stdout,
                stderr=stderr + f"\nProcess timed out after {timeout_minutes} minutes"
            )

        duration = (datetime.now() - start_time).total_seconds()

        # Save results
        save_run_results(run_dir, result, env, duration, config_info)

    log("=" * 50)
    log("Matrix benchmark completed!")
    log("Generating summary...")
    generate_summary(results_dir)


if __name__ == "__main__":
    import sys
    if len(sys.argv) == 1:
        # Default to matrix if no args
        matrix()
    else:
        fire.Fire({
            'single': single,
            'matrix': matrix
        })
