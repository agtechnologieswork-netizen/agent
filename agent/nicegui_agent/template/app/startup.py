import logging
from app.database import create_tables
from nicegui import ui

logger = logging.getLogger(__name__)


def startup() -> None:
    """Application startup - initialize database and UI components"""
    logger.info("Starting application initialization...")
    
    # Create database tables
    create_tables()
    logger.info("Database tables initialized")
    
    # Initialize widget system
    try:
        from app.widget_ui import initialize_widgets, WidgetManager
        initialize_widgets()
        logger.info("Widget system initialized")
    except Exception as e:
        logger.warning(f"Widget system not available: {e}")
    
    # Main page with widget dashboard
    @ui.page("/")
    def index():
        try:
            from app.widget_ui import WidgetManager
            manager = WidgetManager()
            manager.render_dashboard()
        except:
            # Fallback if widget system not available
            ui.label("🚧 Work in progress 🚧").style("font-size: 2rem; text-align: center; margin-top: 2rem")
    
    logger.info("Application startup completed successfully")
