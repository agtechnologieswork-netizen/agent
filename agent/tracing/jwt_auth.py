"""JWT token generation for Neon Data API authentication."""

import jwt
import time
from typing import Optional


def create_neon_jwt(user_id: str, secret_key: str, project_id: str) -> str:
    """
    Generate a JWT token for Neon Data API authentication.
    
    For production, use Neon Auth SDK instead of manual JWT generation.
    """
    payload = {
        'sub': user_id,  # subject (user ID)
        'aud': 'authenticated',  # audience - must be 'authenticated' for Data API
        'role': 'authenticated',  # role for RLS
        'iss': f'https://api.stack-auth.com/api/v1/projects/{project_id}',  # issuer
        'exp': int(time.time()) + 3600,  # expires in 1 hour
        'iat': int(time.time()),  # issued at
        'email': f'{user_id}@example.com',  # optional user email
    }
    
    # Note: The secret_key format depends on your Neon Auth setup
    # You might need to decode it from base64 or use it directly
    return jwt.encode(payload, secret_key, algorithm='HS256')


def get_user_token(user_id: str = 'test-user') -> Optional[str]:
    """Get a JWT token for testing (replace with proper auth flow)."""
    import os
    from dotenv import load_dotenv
    
    load_dotenv()
    
    secret_key = os.getenv('STACK_SECRET_SERVER_KEY')
    project_id = os.getenv('STACK_PROJECT_ID')
    
    if not secret_key or not project_id:
        return None
    
    try:
        return create_neon_jwt(user_id, secret_key, project_id)
    except Exception as e:
        print(f"JWT generation failed: {e}")
        return None