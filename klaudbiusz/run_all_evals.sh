#!/bin/bash
set -e

# Run All Evaluations - Both Vanilla SDK and MCP Mode
# Generates apps and evaluates in both modes for comparison

echo "=========================================="
echo "Run All Evaluations - Vanilla SDK + MCP"
echo "=========================================="
echo ""

# Load environment variables from .env file if it exists
if [ -f .env ]; then
    echo "✅ Loading environment variables from .env"
    export $(grep -v '^#' .env | xargs)
fi

# Record overall start time
OVERALL_START=$(date +%s)
OVERALL_RUN_ID=$(date +%Y%m%d_%H%M%S)

# Check required environment variables
if [ -z "$DATABRICKS_HOST" ] || [ -z "$DATABRICKS_TOKEN" ] || [ -z "$ANTHROPIC_API_KEY" ]; then
    echo "❌ Error: Required environment variables not set:"
    echo "   DATABRICKS_HOST, DATABRICKS_TOKEN, ANTHROPIC_API_KEY"
    echo "   Set them via shell export or create a .env file"
    exit 1
fi

echo "📋 Overall Configuration:"
echo "   Run ID: $OVERALL_RUN_ID"
echo "   Date: $(date)"
echo "   Modes: Vanilla SDK + MCP"
echo ""

# Step 1: Run Vanilla SDK Mode
echo "=========================================="
echo "1/2: Running Vanilla SDK Mode"
echo "=========================================="
echo ""
VANILLA_START=$(date +%s)
./run_vanilla_eval.sh
VANILLA_END=$(date +%s)
VANILLA_DURATION=$((VANILLA_END - VANILLA_START))

# Move results to vanilla-specific directory
mkdir -p results_${OVERALL_RUN_ID}/vanilla
mv evaluation_report.json results_${OVERALL_RUN_ID}/vanilla/ 2>/dev/null || true
mv EVALUATION_REPORT.md results_${OVERALL_RUN_ID}/vanilla/ 2>/dev/null || true
mv run_metadata.json results_${OVERALL_RUN_ID}/vanilla/ 2>/dev/null || true
mv app results_${OVERALL_RUN_ID}/vanilla/ 2>/dev/null || true

echo ""
echo "✅ Vanilla SDK mode complete (${VANILLA_DURATION}s)"
echo ""

# Step 2: Run MCP Mode
echo "=========================================="
echo "2/2: Running MCP Mode"
echo "=========================================="
echo ""
MCP_START=$(date +%s)
./run_mcp_eval.sh
MCP_END=$(date +%s)
MCP_DURATION=$((MCP_END - MCP_START))

# Move results to mcp-specific directory
mkdir -p results_${OVERALL_RUN_ID}/mcp
mv evaluation_report.json results_${OVERALL_RUN_ID}/mcp/ 2>/dev/null || true
mv EVALUATION_REPORT.md results_${OVERALL_RUN_ID}/mcp/ 2>/dev/null || true
mv run_metadata.json results_${OVERALL_RUN_ID}/mcp/ 2>/dev/null || true
mv app results_${OVERALL_RUN_ID}/mcp/ 2>/dev/null || true

echo ""
echo "✅ MCP mode complete (${MCP_DURATION}s)"
echo ""

# Record combined metadata
OVERALL_END=$(date +%s)
OVERALL_DURATION=$((OVERALL_END - OVERALL_START))

cat > results_${OVERALL_RUN_ID}/combined_metadata.json << EOF
{
  "overall_run_id": "$OVERALL_RUN_ID",
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "date_human": "$(date)",
  "total_duration_sec": $OVERALL_DURATION,
  "modes": {
    "vanilla_sdk": {
      "duration_sec": $VANILLA_DURATION,
      "results_dir": "vanilla"
    },
    "mcp": {
      "duration_sec": $MCP_DURATION,
      "results_dir": "mcp"
    }
  },
  "environment": {
    "databricks_host": "$DATABRICKS_HOST",
    "os": "$(uname -s)",
    "hostname": "$(hostname)"
  }
}
EOF

# Create comparison summary
echo "# Combined Evaluation Report" > results_${OVERALL_RUN_ID}/COMPARISON.md
echo "" >> results_${OVERALL_RUN_ID}/COMPARISON.md
echo "Run ID: $OVERALL_RUN_ID" >> results_${OVERALL_RUN_ID}/COMPARISON.md
echo "Date: $(date)" >> results_${OVERALL_RUN_ID}/COMPARISON.md
echo "" >> results_${OVERALL_RUN_ID}/COMPARISON.md
echo "## Duration Comparison" >> results_${OVERALL_RUN_ID}/COMPARISON.md
echo "" >> results_${OVERALL_RUN_ID}/COMPARISON.md
echo "- **Vanilla SDK Mode:** ${VANILLA_DURATION}s ($(($VANILLA_DURATION / 60))m $(($VANILLA_DURATION % 60))s)" >> results_${OVERALL_RUN_ID}/COMPARISON.md
echo "- **MCP Mode:** ${MCP_DURATION}s ($(($MCP_DURATION / 60))m $(($MCP_DURATION % 60))s)" >> results_${OVERALL_RUN_ID}/COMPARISON.md
echo "- **Total:** ${OVERALL_DURATION}s ($(($OVERALL_DURATION / 60))m $(($OVERALL_DURATION % 60))s)" >> results_${OVERALL_RUN_ID}/COMPARISON.md
echo "" >> results_${OVERALL_RUN_ID}/COMPARISON.md
echo "## Results Location" >> results_${OVERALL_RUN_ID}/COMPARISON.md
echo "" >> results_${OVERALL_RUN_ID}/COMPARISON.md
echo "- Vanilla SDK: \`results_${OVERALL_RUN_ID}/vanilla/\`" >> results_${OVERALL_RUN_ID}/COMPARISON.md
echo "- MCP: \`results_${OVERALL_RUN_ID}/mcp/\`" >> results_${OVERALL_RUN_ID}/COMPARISON.md
echo "" >> results_${OVERALL_RUN_ID}/COMPARISON.md

# Create symlink to latest
rm -f results_latest
ln -s results_${OVERALL_RUN_ID} results_latest

# Generate HTML viewer
echo "🌐 Generating HTML viewer..."
python3 cli/generate_html_viewer.py

echo "=========================================="
echo "✅ All Evaluations Complete!"
echo "=========================================="
echo ""
echo "📊 Overall Summary:"
echo "   Run ID: $OVERALL_RUN_ID"
echo "   Total Time: ${OVERALL_DURATION}s ($(($OVERALL_DURATION / 60))m $(($OVERALL_DURATION % 60))s)"
echo ""
echo "   Vanilla SDK: ${VANILLA_DURATION}s"
echo "   MCP Mode: ${MCP_DURATION}s"
echo ""
echo "📁 Results Directory: results_${OVERALL_RUN_ID}/"
echo "   ├── vanilla/"
echo "   │   ├── evaluation_report.json"
echo "   │   ├── EVALUATION_REPORT.md"
echo "   │   ├── run_metadata.json"
echo "   │   └── app/"
echo "   ├── mcp/"
echo "   │   ├── evaluation_report.json"
echo "   │   ├── EVALUATION_REPORT.md"
echo "   │   ├── run_metadata.json"
echo "   │   └── app/"
echo "   ├── combined_metadata.json"
echo "   └── COMPARISON.md"
echo ""
echo "📊 Quick Access: results_latest/ -> results_${OVERALL_RUN_ID}/"
echo ""
echo "🔍 View results:"
echo "   cat results_${OVERALL_RUN_ID}/COMPARISON.md"
echo "   open evaluation_viewer.html"
echo ""
