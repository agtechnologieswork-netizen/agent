#!/bin/bash
# Docker login script to handle authentication with multiple registries

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}Docker Login Script${NC}"

# Function to login to ECR
login_to_ecr() {
    local region="us-west-2"
    local registry="361769577597.dkr.ecr.us-west-2.amazonaws.com"
    
    echo -e "${YELLOW}Attempting ECR login...${NC}"
    
    # Check if AWS CLI is configured
    if ! aws sts get-caller-identity >/dev/null 2>&1; then
        echo -e "${RED}AWS credentials not configured. Please run:${NC}"
        echo "aws configure"
        echo "or set AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY environment variables"
        return 1
    fi
    
    # Get ECR login token and login
    if aws ecr get-login-password --region $region | docker login --username AWS --password-stdin $registry; then
        echo -e "${GREEN}Successfully logged into ECR${NC}"
        return 0
    else
        echo -e "${RED}Failed to login to ECR${NC}"
        return 1
    fi
}

# Function to login to Docker Hub
login_to_dockerhub() {
    echo -e "${YELLOW}Checking Docker Hub authentication...${NC}"
    
    if [ -n "$DOCKER_USERNAME" ] && [ -n "$DOCKER_PASSWORD" ]; then
        echo -e "${YELLOW}Logging into Docker Hub...${NC}"
        echo "$DOCKER_PASSWORD" | docker login --username "$DOCKER_USERNAME" --password-stdin
        echo -e "${GREEN}Successfully logged into Docker Hub${NC}"
    else
        echo -e "${YELLOW}Docker Hub credentials not provided (DOCKER_USERNAME/DOCKER_PASSWORD)${NC}"
        echo -e "${YELLOW}Skipping Docker Hub login${NC}"
    fi
}

# Main execution
main() {
    # Test basic Docker functionality
    if ! docker info >/dev/null 2>&1; then
        echo -e "${RED}Docker is not running or not accessible${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}Docker is running${NC}"
    
    # Login to registries
    login_to_dockerhub
    
    # Only attempt ECR login if AWS credentials are available
    if command -v aws >/dev/null 2>&1; then
        login_to_ecr || echo -e "${YELLOW}ECR login skipped due to missing/invalid AWS credentials${NC}"
    else
        echo -e "${YELLOW}AWS CLI not found, skipping ECR login${NC}"
    fi
    
    echo -e "${GREEN}Docker login process completed${NC}"
}

# Run main function
main "$@"