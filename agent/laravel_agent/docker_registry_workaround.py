"""
Docker Registry Workaround for Dagger

This module provides a workaround for Dagger's inability to use local Docker images directly.
It sets up a local registry and pushes required images to it, allowing Dagger to pull from
the local registry instead of Docker Hub.
"""

import os
import logging
import subprocess
import time
import socket
from contextlib import contextmanager

logger = logging.getLogger(__name__)

REGISTRY_PORT = 5555
REGISTRY_HOST = f"127.0.0.1:{REGISTRY_PORT}"

def is_port_open(host="127.0.0.1", port=REGISTRY_PORT):
    """Check if a port is open"""
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.settimeout(1)
        result = sock.connect_ex((host, port))
        return result == 0

@contextmanager
def local_registry():
    """Context manager to run a local Docker registry"""
    container_name = f"dagger-local-registry-{REGISTRY_PORT}"
    
    # Check if registry is already running
    result = subprocess.run(
        ["docker", "ps", "-q", "-f", f"name={container_name}"],
        capture_output=True,
        text=True
    )
    
    if result.stdout.strip():
        logger.info(f"Local registry already running on port {REGISTRY_PORT}")
        yield REGISTRY_HOST
        return
    
    try:
        # Start local registry
        logger.info(f"Starting local Docker registry on port {REGISTRY_PORT}...")
        subprocess.run(
            [
                "docker", "run", "-d",
                "--name", container_name,
                "-p", f"0.0.0.0:{REGISTRY_PORT}:5000",
                "-e", "REGISTRY_HTTP_ADDR=0.0.0.0:5000",
                "registry:2"
            ],
            check=True,
            capture_output=True
        )
        
        # Wait for registry to be ready
        for _ in range(10):
            if is_port_open():
                logger.info(f"Local registry ready at {REGISTRY_HOST}")
                break
            time.sleep(0.5)
        else:
            raise RuntimeError("Local registry failed to start")
        
        yield REGISTRY_HOST
        
    finally:
        # Note: We don't stop the registry here as it might be used by Dagger
        # It can be stopped manually with: docker stop dagger-local-registry-5555
        pass

def push_to_local_registry(image: str, registry_host: str = REGISTRY_HOST):
    """Push an image to the local registry"""
    local_tag = f"{registry_host}/{image}"
    
    # Tag the image for local registry
    logger.info(f"Tagging {image} as {local_tag}")
    subprocess.run(
        ["docker", "tag", image, local_tag],
        check=True,
        capture_output=True
    )
    
    # Push to local registry
    logger.info(f"Pushing {local_tag} to local registry...")
    subprocess.run(
        ["docker", "push", local_tag],
        check=True,
        capture_output=True
    )
    
    return local_tag

def setup_local_images():
    """Set up local registry and push required images"""
    images = [
        "php:8.2-fpm-alpine",
        "composer:2",
        "alpine/git",
        "postgres:17.0-alpine",
        "alpine:latest"
    ]
    
    with local_registry() as registry_host:
        local_images = {}
        for image in images:
            try:
                # First ensure we have the image locally
                logger.info(f"Pulling {image} if not present...")
                subprocess.run(
                    ["docker", "pull", image],
                    capture_output=True
                )
                
                # Push to local registry
                local_tag = push_to_local_registry(image, registry_host)
                local_images[image] = local_tag
                logger.info(f"✅ {image} available as {local_tag}")
                
            except subprocess.CalledProcessError as e:
                logger.error(f"Failed to set up {image}: {e}")
                # Continue with other images
        
        return local_images

def patch_image_references(original_image: str) -> str:
    """Convert Docker Hub image reference to local registry reference"""
    if os.getenv("USE_LOCAL_REGISTRY") and is_port_open():
        return f"{REGISTRY_HOST}/{original_image}"
    return original_image

if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO)
    setup_local_images()