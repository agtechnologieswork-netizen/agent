#!/bin/bash
# Clean up evaluated apps and reports after archiving
# CAUTION: This will delete all generated apps and evaluation reports!

set -e

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "⚠️  CLEANUP WARNING"
echo "This will delete:"
echo "  - All apps in app/ directory (20 apps)"
echo "  - All evaluation reports (JSON, CSV, MD)"
echo ""
echo "Archive is safe: klaudbiusz_evaluation_*.tar.gz"
echo ""
read -p "Continue? (yes/no): " confirm

if [ "$confirm" != "yes" ]; then
    echo "❌ Cleanup cancelled"
    exit 0
fi

echo ""
echo "🧹 Starting cleanup..."
echo ""

# Change to project root
cd "${PROJECT_ROOT}"

# Count apps before deletion
APP_COUNT=$(find app -mindepth 1 -maxdepth 1 -type d 2>/dev/null | wc -l | tr -d ' ')

# Remove all generated apps
if [ -d "app" ] && [ "$APP_COUNT" -gt 0 ]; then
    echo "📂 Removing ${APP_COUNT} apps from app/ directory..."
    rm -rf app/*/
    echo "   ✅ Removed all apps"
else
    echo "   ℹ️  No apps to remove"
fi

# Remove evaluation reports
echo ""
echo "📄 Removing evaluation reports..."

if [ -f "evaluation_report.json" ]; then
    rm -f evaluation_report.json
    echo "   ✅ Removed evaluation_report.json"
fi

if [ -f "evaluation_report.csv" ]; then
    rm -f evaluation_report.csv
    echo "   ✅ Removed evaluation_report.csv"
fi

if [ -f "EVALUATION_REPORT.md" ]; then
    rm -f EVALUATION_REPORT.md
    echo "   ✅ Removed EVALUATION_REPORT.md"
fi

# Keep the archive, checksum, and documentation
echo ""
echo "✅ Cleanup complete!"
echo ""
echo "Kept (safe):"
echo "  - klaudbiusz_evaluation_*.tar.gz (archive)"
echo "  - klaudbiusz_evaluation_*.tar.gz.sha256 (checksum)"
echo "  - ARCHIVE_README.md"
echo "  - EVALUATION_METHODOLOGY.md"
echo "  - DORA_METRICS.md"
echo "  - evals.md"
echo "  - IMPLEMENTATION_SUMMARY.md"
echo "  - cli/ scripts"
echo ""
echo "Removed:"
echo "  - app/* (${APP_COUNT} apps)"
echo "  - evaluation_report.json"
echo "  - evaluation_report.csv"
echo "  - EVALUATION_REPORT.md"
echo ""
