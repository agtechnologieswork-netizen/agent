#!/usr/bin/env python3

import sys
import os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), 'agent'))

def test_ollama_conditional_behavior():
    """Test that Ollama provider behaves correctly based on environment variables"""
    
    os.environ.pop('OLLAMA_HOST', None)
    os.environ.pop('OLLAMA_API_BASE', None)
    
    try:
        from llm.utils import get_llm_client
        client = get_llm_client(backend="auto", model_name="llama3.2", cache_mode="off")
        print("❌ Expected error when OLLAMA_HOST not set")
        return False
    except ValueError as e:
        if "OLLAMA_HOST or OLLAMA_API_BASE" in str(e):
            print("✓ Correctly requires OLLAMA_HOST environment variable")
        else:
            print(f"❌ Unexpected error: {e}")
            return False
    
    os.environ['OLLAMA_HOST'] = 'http://localhost:11434'
    
    try:
        get_llm_client(backend="auto", model_name="llama3.2", cache_mode="off")
        print("✓ OllamaLLM can be created when OLLAMA_HOST is set")
    except ImportError as e:
        if "ollama package" in str(e):
            print("✓ Gracefully handles missing ollama package")
        else:
            print(f"❌ Unexpected import error: {e}")
            return False
    except Exception as e:
        print(f"❌ Unexpected error with OLLAMA_HOST set: {e}")
        return False
    
    print("\n✅ All conditional Ollama tests passed!")
    return True

if __name__ == "__main__":
    success = test_ollama_conditional_behavior()
    sys.exit(0 if success else 1)
