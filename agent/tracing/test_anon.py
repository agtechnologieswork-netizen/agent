"""Test anonymous access by temporarily disabling RLS."""

print("""
To test the tracer, you can temporarily disable RLS on the table:

-- In your Neon SQL editor, run:
ALTER TABLE llm_traces DISABLE ROW LEVEL SECURITY;

-- Test the tracer, then re-enable:
ALTER TABLE llm_traces ENABLE ROW LEVEL SECURITY;

This will allow anonymous access for testing the basic functionality.
""")

import os
from dotenv import load_dotenv
from postgrest import SyncPostgrestClient


def main():
    """Test with anonymous access (RLS disabled)."""
    load_dotenv()
    
    data_api_url = os.getenv('NEON_DATA_API_URL')
    client = SyncPostgrestClient(data_api_url)
    
    print("Testing anonymous access (requires RLS disabled)...")
    
    try:
        # test insert
        result = client.from_('llm_traces').insert({
            'data': '{"model": "test", "prompt": "hello", "response": "world"}'
        }).execute()
        print(f"✅ Insert successful: {result}")
        
        # test select
        result = client.from_('llm_traces').select('*').limit(1).execute()
        print(f"✅ Select successful: {result}")
        
    except Exception as e:
        print(f"❌ Error: {e}")
        print("\nMake sure to run: ALTER TABLE llm_traces DISABLE ROW LEVEL SECURITY;")


if __name__ == "__main__":
    main()