"""UI for managing widgets"""
import logging
from nicegui import ui, app
from app.widget_service import WidgetService
from app.widget_renderer import WidgetRenderer, WidgetGrid
from app.widget_models import Widget, WidgetType, WidgetSize
from typing import Optional

logger = logging.getLogger(__name__)


class WidgetManager:
    """UI component for managing widgets"""
    
    def __init__(self):
        self.widget_service = WidgetService()
        self.grid = WidgetGrid()
        self.edit_mode = False
        self.current_page = "dashboard"
        self.container = None
    
    def render_dashboard(self):
        """Render the main dashboard with widgets"""
        with ui.column().classes("w-full") as self.container:
            # Header with controls
            with ui.row().classes("w-full justify-between items-center mb-6"):
                ui.label("Dashboard").classes("text-2xl font-bold")
                
                with ui.row().classes("gap-2"):
                    ui.button(
                        "Edit Mode" if not self.edit_mode else "View Mode",
                        icon="edit" if not self.edit_mode else "visibility",
                        on_click=self.toggle_edit_mode
                    ).props("outline")
                    
                    ui.button(
                        "Add Widget",
                        icon="add",
                        on_click=self.show_add_widget_dialog
                    ).props("color=primary")
            
            # Render widgets
            self.refresh_widgets()
    
    def refresh_widgets(self):
        """Refresh the widget display"""
        if hasattr(self, 'widget_container'):
            self.widget_container.clear()
        
        with ui.column().classes("w-full") as self.widget_container:
            widgets = self.widget_service.get_widgets_for_page(self.current_page)
            
            if not widgets:
                with ui.card().classes("w-full p-8 text-center"):
                    ui.icon("dashboard", size="4rem").classes("text-gray-400")
                    ui.label("No widgets yet").classes("text-xl text-gray-600 mt-4")
                    ui.label("Click 'Add Widget' to get started").classes("text-gray-500")
            else:
                self.grid.render(widgets, editable=self.edit_mode)
    
    def toggle_edit_mode(self):
        """Toggle between edit and view mode"""
        self.edit_mode = not self.edit_mode
        ui.notify(f"{'Edit' if self.edit_mode else 'View'} mode activated")
        self.refresh_widgets()
    
    def show_add_widget_dialog(self):
        """Show dialog for adding a new widget"""
        with ui.dialog() as dialog, ui.card().classes("w-96"):
            ui.label("Add New Widget").classes("text-xl font-bold mb-4")
            
            # Widget configuration form
            name_input = ui.input("Widget Name", placeholder="Enter widget name")
            
            type_select = ui.select(
                "Widget Type",
                options={
                    WidgetType.TEXT: "Text",
                    WidgetType.METRIC: "Metric/KPI",
                    WidgetType.CHART: "Chart",
                    WidgetType.TABLE: "Table",
                    WidgetType.BUTTON: "Button",
                    WidgetType.IMAGE: "Image",
                    WidgetType.CARD: "Card"
                },
                value=WidgetType.TEXT
            )
            
            size_select = ui.select(
                "Widget Size",
                options={
                    WidgetSize.SMALL: "Small (25%)",
                    WidgetSize.MEDIUM: "Medium (50%)",
                    WidgetSize.LARGE: "Large (75%)",
                    WidgetSize.FULL: "Full Width (100%)"
                },
                value=WidgetSize.MEDIUM
            )
            
            # Dynamic configuration based on widget type
            config_container = ui.column().classes("w-full")
            
            def update_config_fields():
                config_container.clear()
                with config_container:
                    widget_type = type_select.value
                    
                    if widget_type == WidgetType.TEXT:
                        ui.textarea("Content", placeholder="Enter text or markdown")
                        ui.switch("Enable Markdown")
                    
                    elif widget_type == WidgetType.METRIC:
                        ui.input("Title", placeholder="Metric title")
                        ui.number("Value", value=0)
                        ui.input("Icon", placeholder="Icon name (optional)")
                        ui.number("Change %", placeholder="Change percentage")
                    
                    elif widget_type == WidgetType.CHART:
                        ui.select("Chart Type", options=["line", "bar", "pie"], value="line")
                        ui.input("Title", placeholder="Chart title")
                        ui.switch("Show Legend")
                    
                    elif widget_type == WidgetType.BUTTON:
                        ui.input("Label", placeholder="Button text")
                        ui.input("Icon", placeholder="Icon name (optional)")
                        ui.select("Action", options=["notify", "navigate"], value="notify")
                    
                    elif widget_type == WidgetType.IMAGE:
                        ui.input("Image URL", placeholder="https://...")
                        ui.input("Caption", placeholder="Image caption (optional)")
            
            type_select.on("update:model-value", update_config_fields)
            update_config_fields()
            
            # Action buttons
            with ui.row().classes("w-full justify-end gap-2 mt-4"):
                ui.button("Cancel", on_click=dialog.close).props("flat")
                ui.button(
                    "Add Widget",
                    on_click=lambda: self.add_widget(
                        name_input.value,
                        type_select.value,
                        size_select.value,
                        dialog
                    )
                ).props("color=primary")
        
        dialog.open()
    
    def add_widget(self, name: str, widget_type: WidgetType, size: WidgetSize, dialog):
        """Add a new widget"""
        if not name:
            ui.notify("Please enter a widget name", type="warning")
            return
        
        # Create widget with basic config
        config = self.get_default_config(widget_type)
        
        widget = self.widget_service.create_widget(
            name=name,
            type=widget_type,
            size=size,
            page=self.current_page,
            config=config
        )
        
        ui.notify(f"Widget '{name}' added successfully", type="positive")
        dialog.close()
        self.refresh_widgets()
    
    def get_default_config(self, widget_type: WidgetType) -> dict:
        """Get default configuration for a widget type"""
        configs = {
            WidgetType.TEXT: {
                "content": "New text widget. Click edit to customize.",
                "markdown": False
            },
            WidgetType.METRIC: {
                "title": "New Metric",
                "value": 0,
                "icon": "trending_up"
            },
            WidgetType.CHART: {
                "chart_type": "line",
                "title": "Sample Chart",
                "data": {
                    "x": ["Jan", "Feb", "Mar", "Apr", "May"],
                    "y": [10, 15, 13, 17, 22]
                }
            },
            WidgetType.TABLE: {
                "title": "Sample Table",
                "columns": [
                    {"name": "id", "label": "ID", "field": "id"},
                    {"name": "name", "label": "Name", "field": "name"}
                ],
                "rows": [
                    {"id": 1, "name": "Item 1"},
                    {"id": 2, "name": "Item 2"}
                ]
            },
            WidgetType.BUTTON: {
                "label": "Click Me",
                "action": "notify",
                "message": "Button clicked!"
            },
            WidgetType.IMAGE: {
                "source": "https://via.placeholder.com/400x200",
                "caption": "Sample Image"
            },
            WidgetType.CARD: {
                "title": "Card Title",
                "subtitle": "Card subtitle",
                "content": "<p>Card content goes here</p>"
            }
        }
        return configs.get(widget_type, {})
    
    def edit_widget(self, widget: Widget):
        """Edit an existing widget"""
        with ui.dialog() as dialog, ui.card().classes("w-96"):
            ui.label(f"Edit Widget: {widget.name}").classes("text-xl font-bold mb-4")
            
            name_input = ui.input("Widget Name", value=widget.name)
            
            size_select = ui.select(
                "Widget Size",
                options={
                    WidgetSize.SMALL: "Small (25%)",
                    WidgetSize.MEDIUM: "Medium (50%)",
                    WidgetSize.LARGE: "Large (75%)",
                    WidgetSize.FULL: "Full Width (100%)"
                },
                value=widget.size
            )
            
            # Config editor (simplified - in production, use dynamic forms)
            ui.label("Configuration (JSON)").classes("mt-4")
            config_editor = ui.textarea(
                value=str(widget.config),
                placeholder="Widget configuration"
            ).classes("w-full font-mono text-sm")
            
            with ui.row().classes("w-full justify-end gap-2 mt-4"):
                ui.button("Cancel", on_click=dialog.close).props("flat")
                ui.button(
                    "Delete",
                    on_click=lambda: self.delete_widget(widget, dialog)
                ).props("flat color=negative")
                ui.button(
                    "Save",
                    on_click=lambda: self.save_widget_changes(
                        widget,
                        name_input.value,
                        size_select.value,
                        config_editor.value,
                        dialog
                    )
                ).props("color=primary")
        
        dialog.open()
    
    def save_widget_changes(self, widget: Widget, name: str, size: WidgetSize, config_str: str, dialog):
        """Save changes to a widget"""
        try:
            import json
            config = json.loads(config_str) if config_str else {}
        except:
            config = widget.config
        
        self.widget_service.update_widget(
            widget.id,
            name=name,
            size=size,
            config=config
        )
        
        ui.notify(f"Widget '{name}' updated", type="positive")
        dialog.close()
        self.refresh_widgets()
    
    def delete_widget(self, widget: Widget, dialog=None):
        """Delete a widget with confirmation"""
        def confirm_delete():
            self.widget_service.delete_widget(widget.id)
            ui.notify(f"Widget '{widget.name}' deleted", type="positive")
            if dialog:
                dialog.close()
            confirm_dialog.close()
            self.refresh_widgets()
        
        with ui.dialog() as confirm_dialog, ui.card():
            ui.label(f"Delete '{widget.name}'?").classes("text-lg")
            ui.label("This action cannot be undone.").classes("text-gray-600")
            
            with ui.row().classes("w-full justify-end gap-2 mt-4"):
                ui.button("Cancel", on_click=confirm_dialog.close).props("flat")
                ui.button("Delete", on_click=confirm_delete).props("color=negative")
        
        confirm_dialog.open()


# Page route for widget dashboard
@ui.page("/widgets")
def widgets_page():
    """Widgets management page"""
    manager = WidgetManager()
    manager.render_dashboard()


# Initialize widgets on startup
def initialize_widgets():
    """Initialize widget system on app startup"""
    try:
        WidgetService.initialize_default_widgets()
        logger.info("Widget system initialized")
    except Exception as e:
        logger.error(f"Failed to initialize widgets: {e}")