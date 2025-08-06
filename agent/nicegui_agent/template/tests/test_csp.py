"""Test that Content Security Policy is properly configured for Vue.js"""
import pytest
from fastapi.testclient import TestClient


def test_csp_headers_allow_vue(client: TestClient):
    """Test that CSP headers include unsafe-eval for Vue.js compatibility"""
    response = client.get("/")
    
    # Check that CSP header exists
    assert "Content-Security-Policy" in response.headers
    
    csp = response.headers["Content-Security-Policy"]
    
    # Check that unsafe-eval is allowed (required for Vue.js)
    assert "'unsafe-eval'" in csp, "CSP must include 'unsafe-eval' for Vue.js to work"
    
    # Check that unsafe-inline is allowed (required for NiceGUI)
    assert "'unsafe-inline'" in csp, "CSP must include 'unsafe-inline' for inline scripts"
    
    # Check script-src specifically allows unsafe-eval
    if "script-src" in csp:
        # Find the script-src directive
        directives = csp.split(";")
        script_src = [d for d in directives if d.strip().startswith("script-src")]
        if script_src:
            assert "'unsafe-eval'" in script_src[0], "script-src must include 'unsafe-eval'"


def test_health_endpoint_works(client: TestClient):
    """Test that health endpoint is accessible"""
    response = client.get("/health")
    assert response.status_code == 200
    assert response.json()["status"] == "healthy"