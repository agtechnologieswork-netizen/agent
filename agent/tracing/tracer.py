"""Neon-based LLM query tracer using Data API."""

import os
import json
from datetime import datetime
from typing import Any, Dict, List, Optional
from uuid import uuid4

from postgrest import SyncPostgrestClient
from dotenv import load_dotenv
from jwt_auth import get_user_token


class NeonTracer:
    """Auth-only LLM query tracer using Neon Data API."""
    
    def __init__(self) -> None:
        load_dotenv()
        
        # get data API URL from environment
        self.data_api_url = os.getenv('NEON_DATA_API_URL')
        if not self.data_api_url or self.data_api_url == 'https://your-data-api-endpoint-here':
            raise ValueError('NEON_DATA_API_URL must be set to your actual Data API endpoint from Neon Console')
        
        # neon auth configuration
        self.stack_project_id = os.getenv('STACK_PROJECT_ID')
        self.stack_secret_key = os.getenv('STACK_SECRET_SERVER_KEY')
        
        if not self.stack_project_id or not self.stack_secret_key:
            raise ValueError('STACK_PROJECT_ID and STACK_SECRET_SERVER_KEY are required')
        
        # initialize postgrest client (auth will be added per-request)
        self.client = SyncPostgrestClient(self.data_api_url)
        
    def _get_auth_headers(self) -> Dict[str, str]:
        """get authentication headers with JWT token."""
        token = get_user_token()
        if not token:
            raise ValueError('Failed to generate JWT token')
        
        return {
            'Authorization': f'Bearer {token}',
            'Content-Type': 'application/json'
        }
        
    def trace(
        self, 
        model: str,
        prompt: str, 
        response: str,
        metadata: Optional[Dict[str, Any]] = None,
        latency_ms: Optional[int] = None
    ) -> str:
        """trace an LLM query by storing it in the database."""
        trace_id = str(uuid4())
        
        data = {
            'id': trace_id,
            'timestamp': datetime.utcnow().isoformat(),
            'model': model,
            'prompt': prompt,
            'response': response,
            'metadata': metadata or {},
            'latency_ms': latency_ms
        }
        
        # insert record via data API with auth
        self.client.headers.update(self._get_auth_headers())
        result = self.client.from_('llm_traces').insert({
            'id': trace_id,
            'data': json.dumps(data)
        }).execute()
        
        return trace_id
    
    def get_traces(self, limit: int = 10, offset: int = 0) -> List[Dict[str, Any]]:
        """retrieve LLM traces for the authenticated user."""
        self.client.headers.update(self._get_auth_headers())
        result = self.client.from_('llm_traces')\
            .select('id, created_at, data')\
            .order('created_at', desc=True)\
            .limit(limit)\
            .offset(offset)\
            .execute()
        
        traces = []
        for row in result.data:
            trace = json.loads(row['data'])
            trace['created_at'] = row['created_at']
            traces.append(trace)
        
        return traces
    
    def get_trace(self, trace_id: str) -> Optional[Dict[str, Any]]:
        """retrieve a specific trace by ID."""
        self.client.headers.update(self._get_auth_headers())
        result = self.client.from_('llm_traces')\
            .select('id, created_at, data')\
            .eq('id', trace_id)\
            .execute()
        
        if not result.data:
            return None
        
        row = result.data[0]
        trace = json.loads(row['data'])
        trace['created_at'] = row['created_at']
        
        return trace