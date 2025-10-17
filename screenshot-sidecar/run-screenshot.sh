#!/bin/bash
# Screenshot wrapper script
# Usage: ./run-screenshot.sh <app-directory> <output-path> [env-vars]
# Example: ./run-screenshot.sh ../myapp screenshot.png "PORT=8000,DEBUG=true"

set -e

APP_DIR="${1:-.}"
OUTPUT_PATH="${2:-screenshot.png}"
ENV_VARS="${3:-}"

if [ -n "$ENV_VARS" ]; then
  dagger call \
    screenshot-app \
    --app-source="$APP_DIR" \
    --env-vars="$ENV_VARS" \
    export \
    --path="$OUTPUT_PATH"
else
  dagger call \
    screenshot-app \
    --app-source="$APP_DIR" \
    export \
    --path="$OUTPUT_PATH"
fi

echo "Screenshot saved to $OUTPUT_PATH"
