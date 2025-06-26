#!/bin/bash
# Fix Docker configuration for credential store issues

set -e

echo "Fixing Docker configuration..."

# Create backup of current config
if [ -f ~/.docker/config.json ]; then
    cp ~/.docker/config.json ~/.docker/config.json.backup
    echo "Backed up existing Docker config"
fi

# Create a more robust Docker config
cat > ~/.docker/config.json << 'EOF'
{
  "auths": {
    "${AWS_ACCOUNT_ID}.dkr.ecr.${AWS_REGION}.amazonaws.com": {},
    "${DOCKER_REGISTRY}": {}
    "https://index.docker.io/v1/": {}
  },
  "credStore": "desktop.exe",
  "credsStore": "desktop.exe",
  "experimental": "disabled"
}
EOF

echo "Docker configuration updated successfully"

# Test the configuration
echo "Testing Docker configuration..."
docker info > /dev/null 2>&1 && echo "Docker configuration is working" || echo "Warning: Docker configuration may have issues"