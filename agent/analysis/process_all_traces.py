#!/usr/bin/env python3
"""Process all fsm_exit trace files in a directory"""

import sys
from pathlib import Path
from fire import Fire
from analysis.nicegui_trace_viewer import process_trace_file


def process_traces(input_dir: str, output_dir: str = "logs"):
    """Process all fsm_exit.json files in input directory

    Args:
        input_dir: Directory containing *fsm_exit.json files
        output_dir: Directory to save output files (default: logs/)
    """
    input_path = Path(input_dir)
    output_path = Path(output_dir)

    if not input_path.exists():
        print(f"Error: Input directory {input_path} does not exist")
        sys.exit(1)

    # ensure output directory exists
    output_path.mkdir(parents=True, exist_ok=True)

    # find all fsm_exit files
    fsm_exit_files = list(input_path.rglob("*fsm_exit.json"))

    if not fsm_exit_files:
        print(f"No fsm_exit.json files found in {input_path}")
        return

    print(f"Found {len(fsm_exit_files)} fsm_exit files to process")
    
    processed_count = 0
    skipped_count = 0

    for trace_file in fsm_exit_files:
        # extract trace id based on file structure
        trace_dir = trace_file.parent
        
        if trace_file.name == "fsm_exit.json":
            # files are in subdirectories like: app-UUID.req-UUID_timestamp/fsm_exit.json
            trace_id = trace_dir.name
            display_name = f"{trace_dir.name}/{trace_file.name}"
        else:
            # files are directly in directory like: {trace_id}_{timestamp}-fsm_exit.json
            stem = trace_file.stem  # removes .json
            trace_id = stem.replace("-fsm_exit", "")
            display_name = trace_file.name

        output_file = output_path / f"{trace_id}.txt"

        print(f"Processing {display_name} -> {output_file.name}")

        try:
            # call process_trace_file with proper arguments
            process_trace_file(str(trace_file), output=str(output_file))
            processed_count += 1
        except ValueError as e:
            # handle non-nicegui files quietly
            if "not a NiceGUI trace file" in str(e):
                skipped_count += 1
            else:
                print(f"Error processing {trace_file.name}: {e}")
            continue
        except Exception as e:
            print(f"Error processing {trace_file.name}: {e}")
            # continue with next file instead of crashing
            continue

    print(f"\nProcessed {processed_count} NiceGUI trace files successfully")
    if skipped_count > 0:
        print(f"Skipped {skipped_count} non-NiceGUI trace files")
    print(f"Output saved to {output_path}")


if __name__ == "__main__":
    Fire(process_traces)
