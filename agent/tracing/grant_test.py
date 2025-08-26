"""Test with explicit grants and role information."""

print("""
The Data API might be using a specific role. Try these commands:

-- Check what roles exist:
\\du

-- Grant permissions to common Data API roles:
GRANT ALL ON TABLE llm_traces TO PUBLIC;
GRANT ALL ON TABLE llm_traces TO postgres;
GRANT ALL ON TABLE llm_traces TO neondb_owner;

-- If there's a specific API role, grant to that too:
-- GRANT ALL ON TABLE llm_traces TO [api_role_name];

-- Also try this policy that's more explicit:
DROP POLICY IF EXISTS "Allow public access" ON llm_traces;
CREATE POLICY "Allow all access" ON llm_traces
  FOR ALL 
  TO PUBLIC
  USING (true)
  WITH CHECK (true);
""")

import os
from dotenv import load_dotenv
from postgrest import SyncPostgrestClient


def main():
    """Test access after grants."""
    load_dotenv()
    
    data_api_url = os.getenv('NEON_DATA_API_URL')
    client = SyncPostgrestClient(data_api_url)
    
    print("Testing access after grants...")
    
    try:
        result = client.from_('llm_traces').insert({
            'data': '{"test": "after_grants"}'
        }).execute()
        print(f"✅ Success: {result}")
        
    except Exception as e:
        print(f"❌ Still failing: {e}")


if __name__ == "__main__":
    main()