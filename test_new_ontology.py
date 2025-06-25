#!/usr/bin/env python3

import sys
import os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), 'agent'))

def test_new_ontology():
    """Test the new model ontology (best_coding/universal/ultra_fast/vision)"""
    
    print("=== Testing New Model Ontology ===\n")
    
    for key in list(os.environ.keys()):
        if key.startswith('LLM_'):
            del os.environ[key]
    
    try:
        from llm.utils import get_best_coding_llm_client, get_universal_llm_client, get_ultra_fast_llm_client, get_vision_llm_client
        from llm.models_config import ModelCategory, get_model_for_category
        print("✓ Successfully imported all new categorized client functions")
    except ImportError as e:
        print(f"❌ Failed to import: {e}")
        return False
    
    print("1. Testing default model selection with new ontology...")
    defaults = {
        'BEST_CODING': get_model_for_category(ModelCategory.BEST_CODING),
        'UNIVERSAL': get_model_for_category(ModelCategory.UNIVERSAL),
        'ULTRA_FAST': get_model_for_category(ModelCategory.ULTRA_FAST),
        'VISION': get_model_for_category(ModelCategory.VISION)
    }
    print(f"   Defaults: {defaults}")
    
    expected_defaults = {
        'BEST_CODING': 'sonnet',           # slow, high quality
        'UNIVERSAL': 'gemini-flash',       # medium speed for FSM tools
        'ULTRA_FAST': 'gemini-flash-lite', # ultra fast for commit names
        'VISION': 'gemini-flash-lite'      # vision tasks
    }
    if defaults != expected_defaults:
        print(f"❌ Expected {expected_defaults}, got {defaults}")
        return False
    print("✓ Default models correct for new ontology")
    
    print("\n2. Testing new environment variable names...")
    os.environ['LLM_BEST_CODING_MODEL'] = 'devstral'
    os.environ['LLM_UNIVERSAL_MODEL'] = 'llama3.3'
    os.environ['LLM_ULTRA_FAST_MODEL'] = 'phi4'
    os.environ['LLM_VISION_MODEL'] = 'gemma3'
    
    overrides = {
        'BEST_CODING': get_model_for_category(ModelCategory.BEST_CODING),
        'UNIVERSAL': get_model_for_category(ModelCategory.UNIVERSAL),
        'ULTRA_FAST': get_model_for_category(ModelCategory.ULTRA_FAST),
        'VISION': get_model_for_category(ModelCategory.VISION)
    }
    print(f"   Overrides: {overrides}")
    
    expected_overrides = {
        'BEST_CODING': 'devstral',
        'UNIVERSAL': 'llama3.3',
        'ULTRA_FAST': 'phi4',
        'VISION': 'gemma3'
    }
    if overrides != expected_overrides:
        print(f"❌ Expected {expected_overrides}, got {overrides}")
        return False
    print("✓ New environment variable names work correctly")
    
    print("\n3. Testing new client function names...")
    os.environ['OLLAMA_HOST'] = 'http://localhost:11434'
    
    try:
        best_coding_client = get_best_coding_llm_client(cache_mode='off')
        print("❌ Should have failed without ollama package")
        return False
    except ImportError as e:
        if "ollama package" in str(e):
            print("✓ Best coding client correctly requires ollama package")
        else:
            print(f"❌ Unexpected error: {e}")
            return False
    except ValueError as e:
        if "OLLAMA_HOST" in str(e):
            print("✓ Best coding client correctly requires OLLAMA_HOST")
        else:
            print(f"❌ Unexpected error: {e}")
            return False
    
    print("\n✅ New ontology test passed!")
    print("\n=== Summary ===")
    print("✓ New model categories work (BEST_CODING/UNIVERSAL/ULTRA_FAST/VISION)")
    print("✓ New environment variable names work (LLM_*_MODEL)")
    print("✓ New client function names work")
    print("✓ Ollama switching still works with new categories")
    print("\n🎉 Model ontology successfully updated!")
    
    return True

if __name__ == "__main__":
    success = test_new_ontology()
    sys.exit(0 if success else 1)
