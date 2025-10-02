#!/bin/bash

# Databricks Deployment Script
# Usage: ./deploy-to-dbx-apps.sh <folder_path> [app_name]
#
# Optional Environment Variables:
# - SKIP_USER_DETECTION: Set to "true" to skip user detection and use "user"
#
# The script follows Databricks documentation for workspace paths:
# https://docs.databricks.com/aws/en/dev-tools/databricks-apps/deploy

set -e  # Exit on any error

# Configuration - Replace with your actual values
# DATABRICKS_HOST="<PLACEHOLDER_DBX_HOST>"
# DATABRICKS_TOKEN="<PLACEHOLDER_DBX_TOKEN>"
# # Set environment variables for databricks CLI
# export DATABRICKS_HOST="$DATABRICKS_HOST"
# export DATABRICKS_TOKEN="$DATABRICKS_TOKEN"


# Central LogFood host
# DATABRICKS_HOST="https://adb-2548836972759138.18.azuredatabricks.net/"
# Dogfood host
# DATABRICKS_HOST="https://e2-dogfood.staging.cloud.databricks.com"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}ℹ${NC} $1" >&2
}

log_success() {
    echo -e "${GREEN}✓${NC} $1" >&2
}

log_warning() {
    echo -e "${YELLOW}⚠${NC} $1" >&2
}

log_error() {
    echo -e "${RED}✗${NC} $1" >&2
}

# Function to validate inputs
validate_inputs() {
    if [ -z "$1" ]; then
        log_error "Folder path is required"
        echo "Usage: $0 <folder_path> [app_name]"
        exit 1
    fi

    if [ ! -d "$1" ]; then
        log_error "Folder does not exist: $1"
        exit 1
    fi
}

# Function to check if databricks CLI is installed
check_databricks_cli() {
    log_info "Checking Databricks CLI..."
    if ! command -v databricks &> /dev/null; then
        log_error "Databricks CLI is not installed"
        echo "Install from: https://docs.databricks.com/dev-tools/cli/install.html"
        exit 1
    fi
    log_success "Databricks CLI found"
}

# Function to get current user from Databricks CLI
get_current_user() {
    log_info "Getting current user..."

    local user_json
    user_json=$(databricks current-user me --output json)
    if [ $? -ne 0 ]; then
        log_error "Could not determine current user. Ensure you're authenticated."
        exit 1
    fi

    local user_email
    user_email=$(echo "$user_json" | jq -r '.userName')
    if [ -z "$user_email" ] || [ "$user_email" = "null" ]; then
        log_error "Could not parse current user from response"
        exit 1
    fi

    log_success "Authenticated as: $user_email"
    echo "$user_email"
}

# Function to check if app exists
check_app_exists() {
    local app_name="$1"
    local apps_json

    log_info "Checking if app exists..."
    apps_json=$(databricks apps list --output json 2>&1)
    if [ $? -ne 0 ]; then
        log_error "Failed to list apps"
        echo "CLI output: $apps_json"
        return 1
    fi

    local exists
    exists=$(echo "$apps_json" | jq -r --arg name "$app_name" '[.[] | select(.name == $name)] | length > 0')
    [ "$exists" = "true" ]
}

# Function to create app
create_app() {
    local app_name="$1"
    log_info "Creating app: $app_name"

    databricks apps create "$app_name"
    if [ $? -eq 0 ]; then
        log_success "App created: $app_name"
    else
        log_error "Failed to create app: $app_name"
        exit 1
    fi
}

# Function to import code to workspace
import_code() {
    local source_path="$1"
    local workspace_path="$2"

    log_info "Importing code to workspace: $workspace_path"

    # Create temporary directory for filtered files
    local temp_dir=$(mktemp -d)
    trap "rm -rf $temp_dir" EXIT

    log_info "Filtering files to temporary directory..."

    # Use rsync to copy files while excluding unwanted directories
    rsync -a \
        --exclude='.venv' \
        --exclude='venv' \
        --exclude='node_modules' \
        --exclude='dist' \
        --exclude='build' \
        --exclude='.git' \
        --exclude='.env' \
        --exclude='Dockerfile' \
        --exclude='*.pyc' \
        --exclude='__pycache__' \
        --exclude='.DS_Store' \
        --exclude='.dockerignore' \
        --exclude='.gitignore' \
        --exclude='docker-compose.yml' \
        "$source_path/" "$temp_dir/"

    databricks workspace import-dir --overwrite "$temp_dir" "$workspace_path"
    if [ $? -eq 0 ]; then
        log_success "Code imported to workspace"
    else
        log_error "Failed to import code to workspace"
        exit 1
    fi
}

# Function to deploy app
deploy_app() {
    local app_name="$1"
    local workspace_path="$2"

    log_info "Deploying app: $app_name"

    databricks apps deploy "$app_name" --source-code-path "$workspace_path"
    if [ $? -eq 0 ]; then
        log_success "App deployed successfully"
    else
        log_error "Failed to deploy app"
        exit 1
    fi
}

# Function to get app URL
get_app_url() {
    local app_name="$1"
    local app_json

    app_json=$(databricks apps get "$app_name" --output json 2>&1)
    if [ $? -eq 0 ]; then
        local url
        url=$(echo "$app_json" | jq -r '.url')
        if [ -n "$url" ] && [ "$url" != "null" ]; then
            echo ""
            log_success "App URL: $url"
        fi
    fi
}

# Main deployment function
deploy_to_databricks() {
    local folder_path="$1"
    local app_name="$2"

    log_info "Preparing deployment..."

    # Convert to absolute path
    folder_path=$(realpath "$folder_path")

    # Generate app name if not provided
    if [ -z "$app_name" ]; then
        app_name="app-$(basename "$folder_path" | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9]/-/g')"
    fi

    log_info "App name: $app_name"


    # Get current user for workspace path
    local user_email
    if [ "$SKIP_USER_DETECTION" = "true" ]; then
        user_email="user"
    else
        user_email=$(get_current_user)
    fi

    # Generate workspace path
    local workspace_path="/Workspace/Users/$user_email/$app_name"

    log_info "Deploying: $app_name"
    log_info "Source: $folder_path"
    log_info "Workspace: $workspace_path"

    # Check if app exists, create if it doesn't
    if check_app_exists "$app_name"; then
        log_info "App exists, updating..."
    else
        create_app "$app_name"
    fi

    # Sync code to workspace
    import_code "$folder_path" "$workspace_path"

    # Deploy the app
    deploy_app "$app_name" "$workspace_path"

    # Get and display app URL
    get_app_url "$app_name"

    echo ""
    log_success "Deployment completed!"
}

# Main script execution
main() {
    echo "🚀 Databricks Deployment"
    echo "========================"
    echo ""

    # Validate inputs
    log_info "Validating inputs..."
    validate_inputs "$@"
    log_success "Inputs validated"

    # Check prerequisites
    check_databricks_cli

    # Run deployment
    log_info "Starting deployment process..."
    deploy_to_databricks "$1" "$2"
}

# Run main function with all arguments
main "$@"
