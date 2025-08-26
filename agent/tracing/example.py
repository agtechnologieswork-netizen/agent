"""Example usage of NeonTracer for LLM query tracing."""

import asyncio
from tracer import NeonTracer
from anthropic import Anthropic


async def main():
    """demonstrate LLM query tracing with Anthropic."""
    
    # initialize tracer
    tracer = NeonTracer()
    
    # initialize anthropic client (assumes ANTHROPIC_API_KEY is set)
    anthropic = Anthropic()
    
    # example prompt
    prompt = "What is the capital of France?"
    
    print(f"Sending prompt: {prompt}")
    
    try:
        # make LLM request and trace it
        import time
        start_time = time.time()
        
        message = anthropic.messages.create(
            model="claude-3-haiku-20240307",
            max_tokens=100,
            messages=[{"role": "user", "content": prompt}]
        )
        
        end_time = time.time()
        latency_ms = int((end_time - start_time) * 1000)
        
        response = message.content[0].text
        
        # trace the query
        trace_id = tracer.trace(
            model="claude-3-haiku-20240307",
            prompt=prompt,
            response=response,
            metadata={
                "tokens_input": message.usage.input_tokens,
                "tokens_output": message.usage.output_tokens,
                "finish_reason": message.stop_reason
            },
            latency_ms=latency_ms
        )
        
        print(f"Response: {response}")
        print(f"Trace ID: {trace_id}")
        print(f"Latency: {latency_ms}ms")
        
        # retrieve recent traces
        print("\nRecent traces:")
        traces = tracer.get_traces(limit=3)
        for i, trace in enumerate(traces, 1):
            print(f"{i}. [{trace['timestamp']}] {trace['model']}")
            print(f"   Prompt: {trace['prompt'][:50]}...")
            print(f"   Response: {trace['response'][:50]}...")
            print(f"   Latency: {trace.get('latency_ms', 'N/A')}ms")
            print()
        
    except Exception as e:
        print(f"Error: {e}")


if __name__ == "__main__":
    asyncio.run(main())