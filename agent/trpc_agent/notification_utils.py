import logging
from typing import Callable, Awaitable

logger = logging.getLogger(__name__)


async def notify_if_callback(event_callback: Callable[[str], Awaitable[None]] | None, message: str, error_context: str = "notification") -> None:
    """
    Utility function to send event notifications if callback is available.
    
    Args:
        event_callback: Optional callback function to send events
        message: The message to send to the callback
        error_context: Context description for error logging (default: "notification")
    """
    if event_callback:
        try:
            await event_callback(message)
        except Exception as e:
            logger.warning(f"Failed to emit {error_context}: {e}")