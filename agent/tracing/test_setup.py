"""Test script to check Neon Data API setup and table creation."""

import os
from dotenv import load_dotenv
from postgrest import SyncPostgrestClient


def main():
    """Test the Neon Data API setup."""
    load_dotenv()
    
    data_api_url = os.getenv('NEON_DATA_API_URL')
    stack_secret = os.getenv('STACK_SECRET_SERVER_KEY')
    
    print(f"Data API URL: {data_api_url}")
    print(f"Stack Secret: {stack_secret[:20]}..." if stack_secret else "None")
    
    if not data_api_url:
        print("❌ NEON_DATA_API_URL not set")
        return
    
    print("\n--- Testing Data API Connection ---")
    
    # test without auth first
    try:
        client = SyncPostgrestClient(data_api_url)
        result = client.from_('llm_traces').select('*').limit(1).execute()
        print("✅ Anonymous access works")
        print(f"Response: {result}")
    except Exception as e:
        print(f"❌ Anonymous access failed: {e}")
    
    # test with auth headers (if available)
    if stack_secret:
        print("\n--- Testing with Authentication ---")
        try:
            client = SyncPostgrestClient(
                data_api_url,
                headers={
                    'Authorization': f'Bearer {stack_secret}',
                    'apikey': stack_secret
                }
            )
            result = client.from_('llm_traces').select('*').limit(1).execute()
            print("✅ Authenticated access works")
            print(f"Response: {result}")
        except Exception as e:
            print(f"❌ Authenticated access failed: {e}")
    
    print("\n--- Next Steps ---")
    print("1. Ensure the 'llm_traces' table exists in your database")
    print("2. Set up proper RLS policies if using authentication")
    print("3. Consider enabling anonymous access temporarily for testing")


if __name__ == "__main__":
    main()