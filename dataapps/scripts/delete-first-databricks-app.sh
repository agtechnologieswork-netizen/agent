#!/bin/bash

# Script to get the last (most recent) Databricks app created by the current user and delete it if it exists
# Uses the Databricks CLI to manage apps

set -e  # Exit on any error

echo "🔍 Checking for Databricks apps created by current user..."

# Get current user information
echo "👤 Getting current user information..."
CURRENT_USER_JSON=$(databricks current-user me --output json 2>&1)
if [ $? -ne 0 ]; then
    echo "❌ Could not determine current user. Please ensure you're authenticated with Databricks CLI."
    exit 1
fi

CURRENT_USER=$(echo "$CURRENT_USER_JSON" | jq -r '.userName')
if [ -z "$CURRENT_USER" ] || [ "$CURRENT_USER" = "null" ]; then
    echo "❌ Could not parse current user from response"
    exit 1
fi

echo "👤 Using current user: $CURRENT_USER"

# Get list of apps
echo "📱 Getting list of apps..."
APPS_JSON=$(databricks apps list --output json 2>&1)
if [ $? -ne 0 ]; then
    echo "❌ Failed to retrieve apps list"
    exit 1
fi

# Find the latest app created by current user
echo "🔍 Finding latest app created by $CURRENT_USER..."
LATEST_APP=$(echo "$APPS_JSON" | jq -r --arg user "$CURRENT_USER" '
    [.[] | select(.creator == $user)]
    | sort_by(.create_time)
    | reverse
    | .[0]
    | .name
')

if [ -z "$LATEST_APP" ] || [ "$LATEST_APP" = "null" ]; then
    echo "ℹ️  No apps found created by $CURRENT_USER"
    exit 0
fi

echo "🗑️  Found latest app: $LATEST_APP"
echo "⚠️  Deleting app: $LATEST_APP"

# Delete the app
if databricks apps delete "$LATEST_APP"; then
    echo "✅ Successfully deleted app: $LATEST_APP"
else
    echo "❌ Failed to delete app: $LATEST_APP"
    exit 1
fi