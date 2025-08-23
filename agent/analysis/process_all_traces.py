#!/usr/bin/env python3
"""Process all fsm_exit trace files in a directory"""

import sys
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor, as_completed
from fire import Fire
from tqdm import tqdm
from analysis.trace_processor import process_trace_file


def process_single_trace(trace_file, output_path):
    """Process a single trace file and return results"""
    trace_dir = trace_file.parent
    
    if trace_file.name == "fsm_exit.json":
        trace_id = trace_dir.name
        display_name = f"{trace_dir.name}/{trace_file.name}"
    else:
        stem = trace_file.stem
        trace_id = stem.replace("-fsm_exit", "")
        display_name = trace_file.name
    
    try:
        # process and save the file
        output_file = output_path / f"{trace_id}.txt"
        agent_type = process_trace_file(str(trace_file), output=str(output_file))
        
        # move to correct directory based on agent type
        agent_output_dir = output_path / agent_type
        agent_output_dir.mkdir(exist_ok=True)
        
        final_output_file = agent_output_dir / f"{trace_id}.txt"
        output_file.rename(final_output_file)
        
        return {"status": "success", "agent_type": agent_type, "display_name": display_name}
        
    except ValueError as e:
        # handle files with no messages
        if "No messages found" in str(e):
            return {"status": "skipped", "error": str(e), "display_name": display_name}
        else:
            return {"status": "error", "error": str(e), "display_name": display_name}
    except Exception as e:
        return {"status": "error", "error": str(e), "display_name": display_name}


def process_traces(input_dir: str, output_dir: str = "logs", max_workers: int = 8):
    """Process all fsm_exit.json files in input directory with concurrent processing

    Args:
        input_dir: Directory containing *fsm_exit.json files
        output_dir: Directory to save output files (default: logs/)
        max_workers: Maximum number of concurrent workers (default: 8)
    """
    input_path = Path(input_dir)
    output_path = Path(output_dir)

    if not input_path.exists():
        print(f"Error: Input directory {input_path} does not exist")
        sys.exit(1)

    # ensure output directory exists
    output_path.mkdir(parents=True, exist_ok=True)

    # find all trace files
    fsm_exit_files = list(input_path.rglob("*fsm_exit.json"))

    if not fsm_exit_files:
        print(f"No fsm_exit.json files found in {input_path}")
        return

    print(f"Found {len(fsm_exit_files)} trace files to process")
    print(f"Processing with {max_workers} concurrent workers...")

    processed_count = 0
    skipped_count = 0
    error_count = 0
    agent_type_counts = {}

    # process files concurrently with progress bar
    with tqdm(total=len(fsm_exit_files), desc="Processing traces", unit="files", leave=False) as pbar:
        with ThreadPoolExecutor(max_workers=max_workers) as executor:
            # submit all tasks
            future_to_file = {
                executor.submit(process_single_trace, trace_file, output_path): trace_file 
                for trace_file in fsm_exit_files
            }
            
            # process results as they complete
            for future in as_completed(future_to_file):
                result = future.result()
                
                if result["status"] == "success":
                    processed_count += 1
                    agent_type = result["agent_type"]
                    agent_type_counts[agent_type] = agent_type_counts.get(agent_type, 0) + 1
                elif result["status"] == "skipped":
                    skipped_count += 1
                elif result["status"] == "error":
                    error_count += 1
                    # optionally print errors
                    # print(f"Error processing {result['display_name']}: {result['error']}")
                
                pbar.update(1)

    print(f"Processed {processed_count} trace files successfully")
    if agent_type_counts:
        print("Agent types processed:")
        for agent_type, count in sorted(agent_type_counts.items()):
            print(f"  {agent_type}: {count} files -> {output_path}/{agent_type}/")
    if skipped_count > 0:
        print(f"Skipped {skipped_count} files with no messages")
    if error_count > 0:
        print(f"Errors: {error_count} files had processing errors")
    print(f"Output organized by agent type in: {output_path}")


if __name__ == "__main__":
    Fire(process_traces)
