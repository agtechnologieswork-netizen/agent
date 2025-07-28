import asyncio
import sys
import tempfile
import shutil
from pathlib import Path
from typing import Optional
from fire import Fire

from analysis.process_all_traces import process_traces
from llm.gemini import GeminiLLM
from llm import common


async def analyze_with_gemini_async(
    trace_dir: str = ".",
    output_dir: Optional[str] = None,
    model: str = "gemini-2.5-flash",
) -> None:
    """Process traces and analyze with Gemini

    Args:
        trace_dir: Directory containing *fsm_exit.json files
        output_dir: Directory for outputs (if None, uses temp dir and saves only final results to 'logs/')
        api_key: Gemini API key (or set GEMINI_API_KEY env var)
        model: Gemini model to use
        prompt: Analysis prompt for Gemini
    """
    # determine output directory
    temp_dir = None
    use_temp = output_dir is None

    if use_temp:
        temp_dir = tempfile.mkdtemp(prefix="trace_analysis_")
        actual_output_dir = temp_dir
        final_output_dir = "logs"
        print(f"Using temporary directory: {temp_dir}")
    else:
        actual_output_dir = output_dir
        final_output_dir = output_dir

    try:
        # process all traces first
        print("Processing trace files...")
        process_traces(trace_dir, actual_output_dir)

        # collect all processed files
        output_path = Path(actual_output_dir)
        processed_files = list(output_path.glob("*.txt"))

        if not processed_files:
            print("No processed trace files found")
            if temp_dir:
                print(f"Temp dir contents: {list(output_path.iterdir())}")
            sys.exit(1)

        print(f"\nConcatenating {len(processed_files)} processed files...")

        # concatenate all processed content
        all_content = []
        for file_path in sorted(processed_files):
            all_content.append(f"\n\n{'='*80}\n")
            all_content.append(f"File: {file_path.name}\n")
            all_content.append(f"{'='*80}\n\n")
            all_content.append(file_path.read_text())

        concatenated_content = "".join(all_content)

        # save concatenated content for reference
        concat_file = output_path / "all_traces_concatenated.txt"
        concat_file.write_text(concatenated_content)
        print(f"Saved concatenated traces to {concat_file}")

        # initialize gemini
        gemini = GeminiLLM(
            model_name=model,
        )

        print(f"\nSending to Gemini ({model})...")

        prompt = "Analyze common issues and suggest improvements for AI assistant guideline. Separate two kinds of issues: 1) those where the AI assistant struggles to choose the right action and needs better guidance, and 2) those where the AI assistant is not able to do the right thing due to missing capabilities or external environment limitations. Provide detailed analysis and suggestions for each issue. Do not use markdown."

        # prepare messages
        messages = [
            common.Message(
                role="user",
                content=[
                    common.TextRaw(text=f"{prompt}\n\nHere are the concatenated trace analysis results:\n\n{concatenated_content}")
                ]
            )
        ]

        # send to gemini
        completion = await gemini.completion(
            messages=messages,
            max_tokens=64 * 1024,
            temperature=1,
        )

        # extract response text
        response_text = ""
        for block in completion.content:
            if isinstance(block, common.TextRaw):
                response_text += block.text

        # save response
        response_file = output_path / "gemini_analysis.md"
        response_file.write_text(response_text)

        # if using temp dir, copy results to final location
        if use_temp:
            final_output_path = Path(final_output_dir)
            final_output_path.mkdir(parents=True, exist_ok=True)

            # copy only the final results
            final_response_file = final_output_path / "gemini_analysis.md"
            shutil.copy2(response_file, final_response_file)

            print(f"\nAnalysis complete! Saved to {final_response_file}")
        else:
            print(f"\nAnalysis complete! Saved to {response_file}")
            print(f"Concatenated traces saved to {concat_file}")

        print("\nGemini's Analysis:")
        print("-" * 80)
        print(response_text)

    except Exception as e:
        print(f"Error: {e}")
        sys.exit(1)
    finally:
        # always cleanup temp directory
        if temp_dir:
            try:
                shutil.rmtree(temp_dir)
                print(f"\nCleaned up temporary directory")
            except Exception as e:
                print(f"Warning: Failed to clean up temp directory: {e}")


def analyze_with_gemini(
    trace_dir: str = ".",
    output_dir: Optional[str] = None,
    model: str = "gemini-2.5-flash",
):
    """Synchronous wrapper for analyze_with_gemini_async"""
    asyncio.run(analyze_with_gemini_async(
        trace_dir=trace_dir,
        output_dir=output_dir,
        model=model,
    ))


if __name__ == "__main__":
    Fire(analyze_with_gemini)
