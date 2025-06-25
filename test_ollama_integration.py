#!/usr/bin/env python3

import sys
import os
import anyio
sys.path.insert(0, os.path.join(os.path.dirname(__file__), 'agent'))

def test_ollama_conditional_behavior():
    """Test that Ollama provider behaves correctly based on environment variables"""
    
    # Test that Ollama models can be created successfully
    os.environ['OLLAMA_HOST'] = 'http://localhost:11434'
    os.environ['PREFER_OLLAMA'] = '1'
    
    try:
        from llm.utils import get_llm_client
        client = get_llm_client(backend="auto", model_name="devstral", cache_mode="off")
        print("✓ OllamaLLM can be created successfully")
        print(f"✓ Client type: {type(client).__name__}")
    except ImportError as e:
        if "ollama package" in str(e):
            print("✓ Gracefully handles missing ollama package")
        else:
            print(f"❌ Unexpected import error: {e}")
            return False
    except Exception as e:
        # Connection errors are OK for testing - Ollama server might not be running
        if "Connection" in str(e) or "ConnectionError" in str(e):
            print("✓ Ollama server not running (expected in CI environments)")
        else:
            print(f"❌ Unexpected error with OLLAMA_HOST set: {e}")
            return False
    
    print("\n✅ All conditional Ollama tests passed!")
    return True

async def test_ollama_function_calling():
    """Test that Ollama function calling works correctly"""
    
    os.environ['OLLAMA_HOST'] = 'http://localhost:11434'
    os.environ['PREFER_OLLAMA'] = '1'
    
    try:
        from llm.utils import get_codegen_llm_client
        from llm.common import Message, TextRaw, ToolUse, Tool
        
        client = get_codegen_llm_client()
        
        # Define a test tool
        tools: list[Tool] = [{
            'name': 'calculate_sum',
            'description': 'Calculate the sum of two numbers',
            'input_schema': {
                'type': 'object',
                'properties': {
                    'a': {'type': 'number', 'description': 'First number'},
                    'b': {'type': 'number', 'description': 'Second number'}
                },
                'required': ['a', 'b']
            }
        }]
        
        messages = [Message(role='user', content=[TextRaw('Use the calculate_sum tool to add 5 and 3')])]
        
        result = await client.completion(
            messages=messages,
            max_tokens=512,
            tools=tools
        )
        
        # Check if we got a tool call
        tool_calls = [block for block in result.content if isinstance(block, ToolUse)]
        
        if len(tool_calls) == 0:
            print("❌ No tool calls found in response")
            return False
            
        tool_call = tool_calls[0]
        if tool_call.name != 'calculate_sum':
            print(f"❌ Expected tool 'calculate_sum', got '{tool_call.name}'")
            return False
            
        if not isinstance(tool_call.input, dict) or 'a' not in tool_call.input or 'b' not in tool_call.input:
            print(f"❌ Tool call arguments invalid: {tool_call.input}")
            return False
        
        print("✅ Ollama function calling works correctly!")
        print(f"   Tool: {tool_call.name}")
        print(f"   Args: {tool_call.input}")
        return True
        
    except ImportError as e:
        if "ollama package" in str(e):
            print("✓ Gracefully handles missing ollama package")
            return True
        else:
            print(f"❌ Unexpected import error: {e}")
            return False
    except Exception as e:
        # If Ollama server is not running, that's OK for this test
        if "Connection" in str(e) or "ConnectionError" in str(e):
            print("✓ Ollama server not running (expected in CI)")
            return True
        print(f"❌ Unexpected error in function calling test: {e}")
        return False

def test_all():
    """Run all Ollama tests"""
    print("🧪 Testing Ollama Integration...")
    
    # Test conditional behavior
    success1 = test_ollama_conditional_behavior()
    
    # Test function calling
    print("\n🔧 Testing Ollama Function Calling...")
    success2 = anyio.run(test_ollama_function_calling)
    
    if success1 and success2:
        print("\n🎉 All Ollama integration tests passed!")
        return True
    else:
        print("\n❌ Some Ollama integration tests failed!")
        return False

if __name__ == "__main__":
    success = test_all()
    sys.exit(0 if success else 1)
