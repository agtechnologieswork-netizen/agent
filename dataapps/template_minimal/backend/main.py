"""
Ultra-minimal DataApps starter template.

This is a bare-bones FastAPI app that you can extend for any data application.
Start here and add exactly what you need.
"""

from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware

# Create the FastAPI app
app = FastAPI(title="DataApps Starter", version="1.0.0")

# Enable CORS for frontend integration
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# Simple example endpoint
@app.get("/")
def read_root():
    return {"message": "Hello from DataApps!"}

@app.get("/health")
def health_check():
    return {"status": "ok"}

# TODO: Add your app logic here
# Examples:
#
# Simple counter:
# counter = 0
# 
# @app.get("/counter")
# def get_counter():
#     return {"value": counter}
# 
# @app.post("/counter/increment")
# def increment():
#     global counter
#     counter += 1
#     return {"value": counter}
#
# Data processing with Polars:
# import polars as pl
# df = pl.DataFrame({"data": [1, 2, 3]})
#
# @app.get("/data")
# def get_data():
#     return df.to_dicts()
#
# File serving:
# from fastapi.staticfiles import StaticFiles
# app.mount("/static", StaticFiles(directory="static"), name="static")

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)