"""Test Data API schema detection."""

import os
from dotenv import load_dotenv
from postgrest import SyncPostgrestClient


def main():
    """Test different endpoints and schema access."""
    load_dotenv()
    
    data_api_url = os.getenv('NEON_DATA_API_URL')
    
    print(f"Data API URL: {data_api_url}")
    
    client = SyncPostgrestClient(data_api_url)
    
    # try to get schema information
    print("\n--- Testing root endpoint ---")
    try:
        # some postgrest endpoints expose schema info at root
        result = client.from_('').select('*').execute()
        print(f"Root: {result}")
    except Exception as e:
        print(f"Root error: {e}")
    
    # try common table patterns
    tables_to_test = ['llm_traces', 'public.llm_traces']
    
    for table in tables_to_test:
        print(f"\n--- Testing table: {table} ---")
        try:
            result = client.from_(table).select('*').limit(1).execute()
            print(f"✅ {table} works: {result}")
        except Exception as e:
            print(f"❌ {table} error: {e}")
    
    # try to insert to trigger different errors
    print(f"\n--- Testing insert to llm_traces ---")
    try:
        result = client.from_('llm_traces').insert({
            'data': '{"test": "data"}'
        }).execute()
        print(f"✅ Insert works: {result}")
    except Exception as e:
        print(f"❌ Insert error: {e}")


if __name__ == "__main__":
    main()