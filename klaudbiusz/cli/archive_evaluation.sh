#!/bin/bash
# Archive all evaluated apps with their evaluation reports

set -e

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Create archive name with timestamp
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
ARCHIVE_NAME="klaudbiusz_evaluation_${TIMESTAMP}.tar.gz"
ARCHIVE_PATH="${PROJECT_ROOT}/${ARCHIVE_NAME}"

echo "📦 Creating evaluation archive..."
echo "Archive: ${ARCHIVE_NAME}"
echo ""

# Change to project root
cd "${PROJECT_ROOT}"

# Create archive with all apps and reports
tar -czf "${ARCHIVE_NAME}" \
  --exclude='app/*/node_modules' \
  --exclude='app/*/client/node_modules' \
  --exclude='app/*/server/node_modules' \
  --exclude='app/*/client/dist' \
  --exclude='app/*/server/dist' \
  --exclude='app/*/.next' \
  --exclude='app/*/build' \
  app/ \
  evaluation_report.json \
  evaluation_report.csv \
  EVALUATION_REPORT.md \
  EVALUATION_METHODOLOGY.md \
  DORA_METRICS.md \
  evals.md \
  IMPLEMENTATION_SUMMARY.md

# Get archive size
ARCHIVE_SIZE=$(du -h "${ARCHIVE_NAME}" | cut -f1)

echo "✅ Archive created successfully!"
echo ""
echo "Archive Details:"
echo "  Location: ${ARCHIVE_PATH}"
echo "  Size: ${ARCHIVE_SIZE}"
echo ""

# Show contents summary
echo "Archive Contents:"
tar -tzf "${ARCHIVE_NAME}" | head -20
TOTAL_FILES=$(tar -tzf "${ARCHIVE_NAME}" | wc -l | tr -d ' ')
echo "  ... (${TOTAL_FILES} total files)"
echo ""

# Create checksum
CHECKSUM=$(shasum -a 256 "${ARCHIVE_NAME}" | cut -d' ' -f1)
echo "SHA-256: ${CHECKSUM}" | tee "${ARCHIVE_NAME}.sha256"

echo ""
echo "🎉 Archive complete: ${ARCHIVE_NAME}"
