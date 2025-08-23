import asyncio
import random
from pathlib import Path
from fire import Fire
from llm.gemini import GeminiLLM
from llm import common


async def analyze_with_gemini_async(input_dir: str, model: str = "gemini-2.5-flash") -> None:
    """Analyze processed trace files with Gemini

    Args:
        input_dir: Directory containing processed .txt trace files
        model: Gemini model to use
    """
    input_path = Path(input_dir)

    if not input_path.exists():
        print(f"Error: Input directory {input_path} does not exist")
        return

    # collect all processed files
    processed_files = list(input_path.glob("*.txt"))

    if not processed_files:
        print(f"No .txt files found in {input_path}")
        return

    print(f"Found {len(processed_files)} processed trace files")
    print("Concatenating files with subsampling...")

    # estimate tokens per file and determine sampling strategy
    # rough estimate: 1 token ≈ 3
    #  characters for english text
    max_tokens = 1_000_000
    max_chars = max_tokens * 3

    # first pass: get total size
    total_chars = 0
    file_sizes = []
    for file_path in sorted(processed_files):
        content = file_path.read_text()
        size = len(content)
        file_sizes.append((file_path, content, size))
        total_chars += size

    print(f"Total characters: {total_chars:,} (estimated {total_chars // 4:,} tokens)")

    # determine sampling strategy
    if total_chars <= max_chars:
        print("No subsampling needed - using all files")
        selected_files = file_sizes
    else:
        # calculate sampling ratio
        sampling_ratio = max_chars / total_chars
        print(f"Subsampling ratio: {sampling_ratio:.2%}")

        # randomly sample files to get diverse content
        random.seed(42)  # reproducible sampling
        selected_files = []
        remaining_chars = max_chars

        # sort randomly for diverse sampling
        random.shuffle(file_sizes)

        for file_path, content, size in file_sizes:
            if remaining_chars <= 0:
                break

            if size <= remaining_chars:
                # include full file
                selected_files.append((file_path, content, size))
                remaining_chars -= size
            else:
                # include partial file (from beginning)
                partial_content = content[:remaining_chars]
                selected_files.append((file_path, partial_content, remaining_chars))
                remaining_chars = 0

        print(f"Selected {len(selected_files)} files/portions totaling ~{max_chars // 4:,} tokens")

    # concatenate selected content
    all_content = []
    total_selected_chars = 0
    for file_path, content, _ in selected_files:
        all_content.append(f"\n\n{'=' * 80}\n")
        all_content.append(f"File: {file_path.name}\n")
        all_content.append(f"{'=' * 80}\n\n")
        all_content.append(content)
        total_selected_chars += len(content)

    concatenated_content = "".join(all_content)
    print(f"Final concatenated size: {len(concatenated_content):,} chars (~{len(concatenated_content) // 4:,} tokens)")

    # save concatenated content for reference
    concat_file = input_path / "all_traces_concatenated.txt"
    concat_file.write_text(concatenated_content)
    print(f"Saved concatenated traces to {concat_file}")

    # initialize gemini
    gemini = GeminiLLM(model_name=model)
    print(f"\nSending to Gemini ({model})...")

    prompt = "Analyze common issues and suggest improvements for AI assistant guideline. Separate two kinds of issues: 1) those where the AI assistant struggles to choose the right action and needs better guidance, and 2) those where the AI assistant is not able to do the right thing due to missing capabilities or external environment limitations. Provide detailed analysis and suggestions for each issue. Add examples of failures and recoveries."

    # prepare messages
    messages = [
        common.Message(
            role="user",
            content=[
                common.TextRaw(
                    text=f"{prompt}\n\nHere are the concatenated trace analysis results:\n\n{concatenated_content}"
                )
            ],
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
    response_file = input_path / "gemini_analysis.md"
    response_file.write_text(response_text)
    print(f"\nAnalysis complete! Saved to {response_file}")

    print("\nGemini's Analysis:")
    print("-" * 80)
    print(response_text)


def analyze_with_gemini(input_dir: str, model: str = "gemini-2.5-flash"):
    """Synchronous wrapper for analyze_with_gemini_async"""
    asyncio.run(analyze_with_gemini_async(input_dir=input_dir, model=model))


if __name__ == "__main__":
    Fire(analyze_with_gemini)
