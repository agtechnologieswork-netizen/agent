"""
Ultra-minimal DataApps starter template.

This is a bare-bones FastAPI app that you can extend for any data application.
Start here and add exactly what you need.
"""

import time
from pathlib import Path

from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import FileResponse
from fastapi.staticfiles import StaticFiles

# Create the FastAPI app
app = FastAPI(title="DataApps Starter", version="1.0.0")

# Enable CORS for frontend integration (React Admin compatible)
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
    expose_headers=["X-Total-Count", "Content-Range"],  # React Admin needs these
)

# Root endpoint (will be overridden by frontend in production)


@app.get("/health")
def health_check():
    """Health check endpoint with detailed status."""
    return {
        "status": "ok",
        "timestamp": int(time.time()),
        "service": "dataapps-template",
        "version": "1.0.0",
    }


# TODO: Add your API resources here
# React Admin expects REST endpoints like:
#
# @app.get("/api/{resource}")           # List with pagination, sorting, filtering
# @app.post("/api/{resource}")          # Create new item
# @app.get("/api/{resource}/{id}")      # Get single item
# @app.put("/api/{resource}/{id}")      # Update item
# @app.delete("/api/{resource}/{id}")   # Delete item
#
# Example for a "users" resource:
# @app.get("/api/users")
# def list_users(skip: int = 0, limit: int = 100):
#     # Return users with X-Total-Count header for pagination
#     return users[skip:skip+limit]
#
# Serve React Admin frontend (in production/Docker)
# Get the directory where this file is located and go up to project root
backend_dir = Path(__file__).parent
project_root = backend_dir.parent
frontend_dist = project_root / "frontend" / "dist"
print(f"Backend dir: {backend_dir}")
print(f"Project root: {project_root}")
print(f"Looking for frontend at: {frontend_dist}")
print(f"Frontend dist exists: {frontend_dist.exists()}")
if frontend_dist.exists():
    print(f"Files in dist: {list(frontend_dist.iterdir())}")
    app.mount(
        "/", StaticFiles(directory=str(frontend_dist), html=True), name="frontend"
    )
    print("Mounted frontend at /")
else:
    # Development mode - frontend served by Vite
    @app.get("/")
    def read_root():
        return {
            "message": "DataApps API running in development mode",
            "frontend": "http://localhost:3000",
            "docs": "http://localhost:8000/docs",
        }


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(app, host="0.0.0.0", port=8000)
