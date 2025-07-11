"""
Dagger Cache Warmer

This module pre-warms Dagger's cache by pulling all required images once at startup.
After the initial pull, Dagger will use its internal cache for subsequent operations.
"""

import logging
import dagger
import asyncio
from typing import List, Dict

logger = logging.getLogger(__name__)

REQUIRED_IMAGES = [
    "php:8.2-fpm-alpine",
    "composer:2",
    "alpine/git",
    "postgres:17.0-alpine",
    "alpine:latest"
]

async def warm_dagger_cache(client: dagger.Client) -> Dict[str, bool]:
    """
    Pre-warm Dagger's cache by pulling all required images.
    
    Returns a dict mapping image names to success status.
    """
    results = {}
    
    for image in REQUIRED_IMAGES:
        retry_count = 3
        for attempt in range(retry_count):
            try:
                logger.info(f"Pre-warming Dagger cache for {image} (attempt {attempt + 1}/{retry_count})...")
                # Simply pull the image into Dagger's cache
                container = client.container().from_(image)
                # Execute a simple command to ensure the image is fully cached
                await container.with_exec(["echo", "cache warmed"]).stdout()
                results[image] = True
                logger.info(f"✅ Successfully cached {image}")
                break
            except Exception as e:
                if attempt < retry_count - 1:
                    logger.warning(f"Attempt {attempt + 1} failed for {image}: {e}. Retrying...")
                    await asyncio.sleep(2 ** attempt)  # Exponential backoff
                else:
                    logger.error(f"❌ Failed to cache {image} after {retry_count} attempts: {e}")
                    results[image] = False
    
    return results

async def ensure_dagger_cache():
    """
    Ensure Dagger has all required images cached.
    This should be called once at application startup.
    """
    logger.info("Ensuring Dagger cache is warmed...")
    
    async with dagger.Connection() as client:
        results = await warm_dagger_cache(client)
        
        success_count = sum(1 for success in results.values() if success)
        total_count = len(results)
        
        if success_count == total_count:
            logger.info(f"✅ All {total_count} images successfully cached in Dagger")
        else:
            logger.warning(f"⚠️  Only {success_count}/{total_count} images cached successfully")
            for image, success in results.items():
                if not success:
                    logger.warning(f"  - Failed: {image}")
        
        return results

if __name__ == "__main__":
    import os
    os.environ['OTEL_SDK_DISABLED'] = 'true'
    logging.basicConfig(level=logging.INFO)
    asyncio.run(ensure_dagger_cache())