"""Test JWT token generation with Neon Auth."""

import os
import jwt
import time
from dotenv import load_dotenv
from postgrest import SyncPostgrestClient


def create_test_jwt(project_id: str, secret_key: str) -> str:
    """Create a test JWT token for Neon Auth."""
    payload = {
        'sub': 'test-user-id',  # subject (user ID)
        'aud': project_id,       # audience (project ID)
        'iss': 'stack-auth',     # issuer
        'exp': int(time.time()) + 3600,  # expires in 1 hour
        'iat': int(time.time()),  # issued at
    }
    
    # use the secret key (might need to decode if it's base64)
    return jwt.encode(payload, secret_key, algorithm='HS256')


def main():
    """Test JWT creation and Data API access."""
    load_dotenv()
    
    data_api_url = os.getenv('NEON_DATA_API_URL')
    project_id = os.getenv('STACK_PROJECT_ID')
    secret_key = os.getenv('STACK_SECRET_SERVER_KEY')
    
    print(f"Project ID: {project_id}")
    print(f"Data API URL: {data_api_url}")
    
    if not all([data_api_url, project_id, secret_key]):
        print("❌ Missing required environment variables")
        return
    
    # create test JWT
    try:
        token = create_test_jwt(project_id, secret_key)
        print(f"✅ Created JWT: {token[:50]}...")
        
        # test with JWT
        client = SyncPostgrestClient(
            data_api_url,
            headers={
                'Authorization': f'Bearer {token}',
                'Content-Type': 'application/json'
            }
        )
        
        print("\n--- Testing table access ---")
        result = client.from_('llm_traces').select('*').limit(1).execute()
        print(f"✅ Success: {result}")
        
    except Exception as e:
        print(f"❌ Error: {e}")


if __name__ == "__main__":
    main()