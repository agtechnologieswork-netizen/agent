"""Final test with correct role permissions."""

print("""
Now grant permissions to the correct Data API roles:

-- Grant table permissions to Data API roles
GRANT ALL ON TABLE llm_traces TO anonymous;
GRANT ALL ON TABLE llm_traces TO authenticated;

-- Grant usage on schema
GRANT USAGE ON SCHEMA public TO anonymous;
GRANT USAGE ON SCHEMA public TO authenticated;

-- Update RLS policy for these roles
DROP POLICY IF EXISTS "Allow public access" ON llm_traces;
DROP POLICY IF EXISTS "Allow all access" ON llm_traces;

CREATE POLICY "Allow anonymous access" ON llm_traces
  FOR ALL 
  TO anonymous
  USING (true)
  WITH CHECK (true);

CREATE POLICY "Allow authenticated access" ON llm_traces
  FOR ALL 
  TO authenticated
  USING (true)
  WITH CHECK (true);
""")

import os
from dotenv import load_dotenv
from postgrest import SyncPostgrestClient


def main():
    """Test access with correct roles."""
    load_dotenv()
    
    data_api_url = os.getenv('NEON_DATA_API_URL')
    client = SyncPostgrestClient(data_api_url)
    
    print("Testing with anonymous/authenticated role permissions...")
    
    try:
        # test insert
        result = client.from_('llm_traces').insert({
            'data': '{"model": "test", "prompt": "What is 2+2?", "response": "4"}'
        }).execute()
        print(f"✅ Insert successful: {result}")
        
        # test select
        result = client.from_('llm_traces').select('*').limit(2).execute()
        print(f"✅ Select successful: {len(result.data)} records")
        for record in result.data:
            print(f"   - ID: {record['id']}, Created: {record['created_at']}")
        
    except Exception as e:
        print(f"❌ Error: {e}")


if __name__ == "__main__":
    main()