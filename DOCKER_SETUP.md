# Docker Setup Guide

## Using Colima

If you're using Colima instead of Docker Desktop, you need to set the `DOCKER_HOST` environment variable.

### Option 1: Set in .env file (Recommended)

Add the following to your `agent/.env` file:

```bash
DOCKER_HOST=unix://$HOME/.colima/docker.sock
```

### Option 2: Export in your shell

Add to your shell profile (~/.zshrc or ~/.bashrc):

```bash
export DOCKER_HOST="unix://$HOME/.colima/docker.sock"
```

### Option 3: Set per command

```bash
DOCKER_HOST="unix://$HOME/.colima/docker.sock" uv run generate "your prompt" --template_id=laravel_agent
```

## Troubleshooting

If you see "Failed to connect to Docker daemon" errors:

1. Make sure Colima is running: `colima status`
2. If not running: `colima start`
3. Verify Docker works: `docker ps`
4. Check the socket exists: `ls -la ~/.colima/docker.sock`