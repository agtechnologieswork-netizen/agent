#!/usr/bin/env python3
"""
Development server runner for the dataapps backend.
This script runs the FastAPI backend with auto-reload enabled.
"""

import uvicorn

if __name__ == "__main__":
    uvicorn.run(
        "main:app",
        host="0.0.0.0",
        port=8000,
        reload=True,
        reload_dirs=[".", "../frontend/reactadmin/src"],
        log_level="info",
    )
