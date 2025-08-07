"""
Widget Generator for creating data-driven widgets programmatically.
This module is used by the agent during code generation and migration.
"""

from typing import Dict, Any, List, Optional
from app.widget_models import WidgetType, WidgetSize
from app.widget_service import WidgetService
import logging

logger = logging.getLogger(__name__)


class WidgetGenerator:
    """Generate widgets programmatically for dashboards"""
    
    @staticmethod
    def create_metric_widget(
        name: str,
        title: str,
        value: Any,
        icon: str = "trending_up",
        change_percent: Optional[float] = None,
        size: WidgetSize = WidgetSize.SMALL,
        page: str = "dashboard"
    ) -> None:
        """Create a metric/KPI widget"""
        config = {
            "title": title,
            "value": value,
            "icon": icon,
        }
        if change_percent is not None:
            config["change"] = change_percent
            
        WidgetService().create_widget(
            name=name,
            type=WidgetType.METRIC,
            size=size,
            page=page,
            config=config
        )
        logger.info(f"Created metric widget: {name}")
    
    @staticmethod
    def create_chart_widget(
        name: str,
        title: str,
        chart_type: str = "line",
        data: Optional[Dict[str, List]] = None,
        size: WidgetSize = WidgetSize.MEDIUM,
        page: str = "dashboard",
        data_source: Optional[Dict] = None
    ) -> None:
        """Create a chart widget"""
        config = {
            "title": title,
            "chart_type": chart_type,
            "show_legend": True,
        }
        
        if data:
            config["data"] = data
        
        if data_source:
            config["data_source"] = data_source
            
        WidgetService().create_widget(
            name=name,
            type=WidgetType.CHART,
            size=size,
            page=page,
            config=config
        )
        logger.info(f"Created chart widget: {name}")
    
    @staticmethod
    def create_table_widget(
        name: str,
        title: str,
        columns: List[Dict],
        rows: List[Dict],
        size: WidgetSize = WidgetSize.LARGE,
        page: str = "dashboard",
        data_source: Optional[Dict] = None
    ) -> None:
        """Create a table widget"""
        config = {
            "title": title,
            "columns": columns,
            "rows": rows,
        }
        
        if data_source:
            config["data_source"] = data_source
            
        WidgetService().create_widget(
            name=name,
            type=WidgetType.TABLE,
            size=size,
            page=page,
            config=config
        )
        logger.info(f"Created table widget: {name}")
    
    @staticmethod
    def create_text_widget(
        name: str,
        content: str,
        markdown: bool = True,
        size: WidgetSize = WidgetSize.MEDIUM,
        page: str = "dashboard"
    ) -> None:
        """Create a text/markdown widget"""
        config = {
            "content": content,
            "markdown": markdown,
        }
        
        WidgetService().create_widget(
            name=name,
            type=WidgetType.TEXT,
            size=size,
            page=page,
            config=config
        )
        logger.info(f"Created text widget: {name}")
    
    @staticmethod
    def generate_sample_widgets():
        """Generate sample widgets for demonstration"""
        try:
            # Welcome text widget
            WidgetGenerator.create_text_widget(
                name="Welcome Message",
                content="""
## 👋 Welcome to Your Custom Dashboard!

This dashboard includes **customizable widgets** that you can:
- ✏️ Edit in real-time
- ➕ Add new widgets
- 🗑️ Delete unwanted widgets
- 📊 Connect to data sources

Toggle **Edit Widgets** mode to start customizing!
                """,
                markdown=True,
                size=WidgetSize.FULL
            )
            
            # Sample metric widgets
            WidgetGenerator.create_metric_widget(
                name="Total Revenue",
                title="Total Revenue",
                value=125430,
                icon="attach_money",
                change_percent=12.5,
                size=WidgetSize.SMALL
            )
            
            WidgetGenerator.create_metric_widget(
                name="Active Users",
                title="Active Users",
                value=1847,
                icon="people",
                change_percent=5.2,
                size=WidgetSize.SMALL
            )
            
            WidgetGenerator.create_metric_widget(
                name="Conversion Rate",
                title="Conversion Rate",
                value="3.4%",
                icon="trending_up",
                change_percent=-2.1,
                size=WidgetSize.SMALL
            )
            
            WidgetGenerator.create_metric_widget(
                name="Avg Order Value",
                title="Avg Order Value",
                value="$67.89",
                icon="shopping_cart",
                change_percent=8.7,
                size=WidgetSize.SMALL
            )
            
            # Sample chart widget
            WidgetGenerator.create_chart_widget(
                name="Monthly Sales Trend",
                title="Monthly Sales Trend",
                chart_type="line",
                data={
                    "x": ["Jan", "Feb", "Mar", "Apr", "May", "Jun"],
                    "y": [45000, 52000, 48000, 61000, 58000, 67000]
                },
                size=WidgetSize.LARGE
            )
            
            # Sample table widget
            WidgetGenerator.create_table_widget(
                name="Top Products",
                title="Top Performing Products",
                columns=[
                    {"name": "product", "label": "Product", "field": "product"},
                    {"name": "sales", "label": "Sales", "field": "sales"},
                    {"name": "revenue", "label": "Revenue", "field": "revenue"},
                ],
                rows=[
                    {"product": "Widget Pro", "sales": 234, "revenue": "$23,400"},
                    {"product": "Dashboard Plus", "sales": 189, "revenue": "$18,900"},
                    {"product": "Analytics Suite", "sales": 156, "revenue": "$31,200"},
                ],
                size=WidgetSize.MEDIUM
            )
            
            logger.info("Sample widgets generated successfully")
            
        except Exception as e:
            logger.error(f"Failed to generate sample widgets: {e}")
    
    @staticmethod
    def clear_all_widgets(page: str = "dashboard"):
        """Clear all widgets for a specific page"""
        try:
            service = WidgetService()
            widgets = service.get_widgets_for_page(page)
            for widget in widgets:
                if widget.id:
                    service.delete_widget(widget.id)
            logger.info(f"Cleared all widgets for page: {page}")
        except Exception as e:
            logger.error(f"Failed to clear widgets: {e}")