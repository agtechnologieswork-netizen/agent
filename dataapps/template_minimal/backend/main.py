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

# Root endpoint (will be overridden by static files in production)

@app.get("/health")
def health_check():
    """Health check endpoint with detailed status."""
    return {
        "status": "ok",
        "timestamp": int(time.time()),
        "service": "dataapps-template",
        "version": "1.0.0"
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
static_dir = Path("static")
if static_dir.exists():
    app.mount("/static", StaticFiles(directory="static"), name="static")

    # Serve React Admin at root path
    @app.get("/")
    async def serve_react_admin():
        return FileResponse("static/index.html")

    # Catch-all for React Admin routes
    @app.get("/{path:path}")
    async def serve_react_admin_routes(path: str):
        # Don't serve static files through this route
        if path.startswith(("api/", "health", "docs", "redoc", "openapi.json")):
            # Let FastAPI handle these routes normally
            raise HTTPException(404)
        return FileResponse("static/index.html")
else:
    # Development mode - frontend served by Vite
    @app.get("/")
    def read_root():
        return {
            "message": "DataApps API running in development mode",
            "frontend": "http://localhost:3000",
            "docs": "http://localhost:8000/docs"
        }

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)
