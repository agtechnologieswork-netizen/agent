# Multi-stage Dockerfile for DataApps template
# Stage 1: Build frontend
FROM node:20-alpine AS frontend-builder

WORKDIR /app
COPY package.json ./
COPY frontend/package.json ./frontend/

# Install frontend dependencies
RUN npm install --prefix frontend

# Copy frontend source and build
COPY frontend/ ./frontend/
RUN npm run build:frontend

# Stage 2: Python backend with uv
FROM python:3.12-slim

# Install system dependencies and uv
RUN apt-get update && apt-get install -y \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && pip install uv

WORKDIR /app

# Copy backend files
COPY backend/ ./backend/

# Install Python dependencies with uv
WORKDIR /app/backend
RUN uv sync --no-install-project

# Copy built frontend from previous stage
WORKDIR /app
RUN mkdir -p frontend
COPY --from=frontend-builder /app/frontend/dist/ ./frontend/dist/

# Create non-root user for security
RUN adduser --disabled-password --gecos '' appuser && \
    chown -R appuser:appuser /app
USER appuser

# Set working directory back to backend for running the app
WORKDIR /app/backend

# Expose port
EXPOSE 8000

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8000/health || exit 1

# Start the application
CMD ["uv", "run", "uvicorn", "main:app", "--host", "0.0.0.0", "--port", "8000"]