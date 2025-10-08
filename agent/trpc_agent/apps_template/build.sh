#!/bin/bash
set -e

echo "Building frontend..."
cd client
npm run build
cd ..

echo "Moving frontend build to server/dist..."
rm -rf server/dist
mv client/dist server/dist

echo "Build completed successfully!"
