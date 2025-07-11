"""
Workaround for Docker timeout issues by using alternative approaches
"""
import os
import subprocess
import logging

logger = logging.getLogger(__name__)

def pull_images_locally():
    """Pre-pull images using local Docker to bypass Dagger timeout issues"""
    images = [
        "php:8.2-fpm-alpine",
        "postgres:17.0-alpine",
        "composer:2",
        "alpine:latest"
    ]
    
    logger.info("Pre-pulling images using local Docker...")
    for image in images:
        try:
            # Check if image already exists locally
            check_result = subprocess.run(
                ['docker', 'images', '-q', image], 
                capture_output=True, 
                text=True
            )
            
            if check_result.stdout.strip():
                logger.info(f"✅ Image {image} already exists locally")
            else:
                logger.info(f"Pulling {image}...")
                result = subprocess.run(
                    ['docker', 'pull', image],
                    capture_output=True,
                    text=True
                )
                if result.returncode == 0:
                    logger.info(f"✅ Successfully pulled {image}")
                else:
                    logger.error(f"Failed to pull {image}: {result.stderr}")
        except Exception as e:
            logger.error(f"Error handling {image}: {e}")

def configure_dagger_for_local_images():
    """Configure Dagger to prefer local images and increase timeouts"""
    # Set environment variables for Dagger to work better with local images
    env_vars = {
        'DOCKER_BUILDKIT': '1',
        'BUILDKIT_PROGRESS': 'plain',
        'DAGGER_VERBOSE': '2',
        'DAGGER_ENGINE_TIMEOUT': '1800',  # 30 minutes
        'OTEL_SDK_DISABLED': 'true',
        # Tell Dagger to use local Docker daemon
        'DAGGER_CACHE_FROM': 'type=local,src=/tmp/dagger-cache',
        'DAGGER_CACHE_TO': 'type=local,dest=/tmp/dagger-cache',
        # Network timeout settings
        'COMPOSE_HTTP_TIMEOUT': '600',
        'DOCKER_CLIENT_TIMEOUT': '600',
    }
    
    for key, value in env_vars.items():
        os.environ[key] = value
        logger.info(f"Set {key}={value}")
    
    # Create cache directory if it doesn't exist
    os.makedirs('/tmp/dagger-cache', exist_ok=True)

def fix_docker_timeout():
    """Main function to fix Docker timeout issues"""
    logger.info("Applying Docker timeout workaround...")
    
    # Step 1: Configure environment
    configure_dagger_for_local_images()
    
    # Step 2: Pre-pull images locally
    pull_images_locally()
    
    # Step 3: Warm Dagger cache if enabled
    if os.getenv('WARM_DAGGER_CACHE', 'true').lower() == 'true':
        try:
            from laravel_agent.dagger_cache_warmer import ensure_dagger_cache
            import asyncio
            logger.info("Warming Dagger cache with required images...")
            results = asyncio.run(ensure_dagger_cache())
            success_count = sum(1 for success in results.values() if success)
            logger.info(f"Dagger cache warmed with {success_count}/{len(results)} images")
        except Exception as e:
            logger.warning(f"Failed to warm Dagger cache: {e}")
            logger.warning("Dagger will pull images on demand")
    
    logger.info("Docker timeout workaround applied!")