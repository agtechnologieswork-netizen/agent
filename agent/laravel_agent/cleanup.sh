#!/bin/bash

# Script to cleanup Laravel containers and volumes

echo "🧹 Cleaning up Laravel containers..."

# Stop and remove containers
echo "Stopping Laravel containers..."
docker stop laravel-app laravel-postgres 2>/dev/null || true

echo "Removing Laravel containers..."
docker rm laravel-app laravel-postgres 2>/dev/null || true

# Optional: Remove volumes (uncomment if you want to clean volumes too)
# echo "Removing Laravel volumes..."
# docker volume rm laravel_postgres_data 2>/dev/null || true

# Optional: Remove Laravel network (uncomment if needed)
# echo "Removing Laravel network..."
# docker network rm laravel_default 2>/dev/null || true

echo "✅ Laravel containers cleaned up successfully!"

# Show remaining containers
echo ""
echo "Remaining containers:"
docker ps -a --format "table {{.Names}}\t{{.Status}}\t{{.Image}}"