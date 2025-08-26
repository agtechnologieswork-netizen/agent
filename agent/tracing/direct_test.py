"""Direct test of Data API endpoints."""

import os
import requests
from dotenv import load_dotenv


def main():
    """Test direct HTTP requests to Data API."""
    load_dotenv()
    
    data_api_url = os.getenv('NEON_DATA_API_URL')
    
    print(f"Testing: {data_api_url}")
    
    # test OpenAPI spec
    print("\n--- Testing OpenAPI spec ---")
    try:
        response = requests.get(f"{data_api_url}/")
        print(f"Status: {response.status_code}")
        if response.status_code == 200:
            spec = response.json()
            if 'paths' in spec:
                print("Available paths:", list(spec['paths'].keys()))
    except Exception as e:
        print(f"Error: {e}")
    
    # try different table access patterns
    endpoints = [
        f"{data_api_url}/llm_traces",
        f"{data_api_url}/llm_traces?select=*&limit=1",
        f"{data_api_url}/rest/v1/llm_traces",
    ]
    
    for endpoint in endpoints:
        print(f"\n--- Testing: {endpoint} ---")
        try:
            response = requests.get(endpoint)
            print(f"Status: {response.status_code}")
            print(f"Response: {response.text[:200]}...")
        except Exception as e:
            print(f"Error: {e}")
    
    # test POST to trigger schema detection
    print(f"\n--- Testing POST to llm_traces ---")
    try:
        response = requests.post(
            f"{data_api_url}/llm_traces",
            json={"data": {"test": "value"}},
            headers={"Content-Type": "application/json"}
        )
        print(f"POST Status: {response.status_code}")
        print(f"POST Response: {response.text}")
    except Exception as e:
        print(f"POST Error: {e}")


if __name__ == "__main__":
    main()