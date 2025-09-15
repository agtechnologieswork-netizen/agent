#!/bin/bash

# Databricks Deployment Script
# Usage: ./deploy-to-dbx.sh <folder_path> [app_name]

set -e  # Exit on any error

# Configuration - Replace with your actual values
DATABRICKS_HOST="<PLACEHOLDER_DBX_HOST>"
DATABRICKS_TOKEN="<PLACEHOLDER_DBX_TOKEN>"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to validate inputs
validate_inputs() {
    if [ -z "$1" ]; then
        log_error "Folder path is required"
        echo "Usage: $0 <folder_path> [app_name]"
        exit 1
    fi

    if [ ! -d "$1" ]; then
        log_error "Folder path does not exist: $1"
        exit 1
    fi

    if [ -z "$DATABRICKS_HOST" ] || [ "$DATABRICKS_HOST" = "your-databricks-host" ]; then
        log_error "DATABRICKS_HOST is not configured. Please update the script with your Databricks host."
        exit 1
    fi

    if [ -z "$DATABRICKS_TOKEN" ] || [ "$DATABRICKS_TOKEN" = "your-databricks-token" ]; then
        log_error "DATABRICKS_TOKEN is not configured. Please update the script with your Databricks token."
        exit 1
    fi
}

# Function to check if databricks CLI is installed
check_databricks_cli() {
    if ! command -v databricks &> /dev/null; then
        log_error "Databricks CLI is not installed or not in PATH"
        echo "Please install it from: https://docs.databricks.com/dev-tools/cli/install.html"
        exit 1
    fi
}

# Function to check if app exists
check_app_exists() {
    local app_name="$1"
    log_info "Checking if app '$app_name' exists..."
    
    if databricks apps list | grep -q "$app_name"; then
        return 0  # App exists
    else
        return 1  # App doesn't exist
    fi
}

# Function to create app if it doesn't exist
create_app() {
    local app_name="$1"
    log_info "Creating Databricks app: $app_name"
    
    if databricks apps create "$app_name"; then
        log_success "App '$app_name' created successfully"
    else
        log_error "Failed to create app '$app_name'"
        exit 1
    fi
}

# Function to import code to workspace
import_code() {
    local source_path="$1"
    local workspace_path="$2"
    
    log_info "Importing code to Databricks workspace..."
    log_info "Source: $source_path"
    log_info "Destination: $workspace_path"
    
    if databricks workspace import-dir --overwrite "$source_path" "$workspace_path"; then
        log_success "Code imported successfully to $workspace_path"
    else
        log_error "Failed to import code to workspace"
        exit 1
    fi
}

# Function to deploy app
deploy_app() {
    local app_name="$1"
    local workspace_path="$2"
    
    log_info "Deploying app '$app_name' from $workspace_path"
    
    if databricks apps deploy "$app_name" --source-code-path "/Workspace$workspace_path"; then
        log_success "App '$app_name' deployed successfully"
    else
        log_error "Failed to deploy app '$app_name'"
        exit 1
    fi
}

# Function to get app URL
get_app_url() {
    local app_name="$1"
    
    log_info "Getting app URL..."
    
    local app_url
    if app_url=$(databricks apps get "$app_name" | jq -r '.url' 2>/dev/null); then
        if [ "$app_url" != "null" ] && [ -n "$app_url" ]; then
            log_success "App URL: $app_url"
            echo "$app_url"
        else
            log_warning "Could not retrieve app URL"
        fi
    else
        log_warning "Could not retrieve app URL (jq might not be installed)"
    fi
}

# Main deployment function
deploy_to_databricks() {
    local folder_path="$1"
    local app_name="$2"
    
    # Convert to absolute path
    folder_path=$(realpath "$folder_path")
    
    # Generate app name if not provided
    if [ -z "$app_name" ]; then
        app_name="app-$(basename "$folder_path" | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9]/-/g')"
    fi
    
    # Generate workspace path
    local workspace_path="/$app_name"
    
    log_info "Starting Databricks deployment"
    log_info "Folder: $folder_path"
    log_info "App Name: $app_name"
    log_info "Workspace Path: $workspace_path"
    log_info "Databricks Host: $DATABRICKS_HOST"
    
    # Set environment variables for databricks CLI
    export DATABRICKS_HOST="$DATABRICKS_HOST"
    export DATABRICKS_TOKEN="$DATABRICKS_TOKEN"
    
    # Check if app exists, create if it doesn't
    if check_app_exists "$app_name"; then
        log_info "App '$app_name' already exists, proceeding with deployment"
    else
        create_app "$app_name"
    fi
    
    # Import code to workspace
    import_code "$folder_path" "$workspace_path"
    
    # Deploy the app
    deploy_app "$app_name" "$workspace_path"
    
    # Get and display app URL
    get_app_url "$app_name"
    
    log_success "Deployment completed successfully!"
}

# Main script execution
main() {
    log_info "Databricks Deployment Script"
    echo "=================================="
    
    # Validate inputs
    validate_inputs "$@"
    
    # Check prerequisites
    check_databricks_cli
    
    # Run deployment
    deploy_to_databricks "$1" "$2"
}

# Run main function with all arguments
main "$@"
