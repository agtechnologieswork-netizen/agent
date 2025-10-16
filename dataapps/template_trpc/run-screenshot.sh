#!/bin/bash
set -e

# generate unique identifiers for concurrent execution
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RANDOM_ID=$(openssl rand -hex 4)
PROJECT_NAME="dataapps-test-${TIMESTAMP}-${RANDOM_ID}"
SCREENSHOT_DIR="./screenshots/temp-${TIMESTAMP}-${RANDOM_ID}"

# create screenshot directory
mkdir -p "${SCREENSHOT_DIR}"

echo "Starting screenshot capture for project: ${PROJECT_NAME}"

# run docker-compose with isolated project and cleanup after
COMPOSE_PROJECT_NAME="${PROJECT_NAME}" \
SCREENSHOT_DIR="${SCREENSHOT_DIR}" \
DATABRICKS_HOST="${DATABRICKS_HOST}" \
DATABRICKS_TOKEN="${DATABRICKS_TOKEN}" \
docker-compose -f docker-compose.test.yml -p "${PROJECT_NAME}" up --abort-on-container-exit --exit-code-from playwright

# cleanup
echo "Cleaning up containers..."
docker-compose -f docker-compose.test.yml -p "${PROJECT_NAME}" down

# move screenshot to app root and cleanup temp directory
if [ -f "${SCREENSHOT_DIR}/screenshot.png" ]; then
  mv "${SCREENSHOT_DIR}/screenshot.png" ./screenshot.png
  rm -rf "${SCREENSHOT_DIR}"
  echo "Screenshot saved to: ./screenshot.png"
else
  echo "Error: Screenshot was not generated"
  rm -rf "${SCREENSHOT_DIR}"
  exit 1
fi
