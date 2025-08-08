#!/usr/bin/env python
"""Test the NiceGUI BI Dashboard application"""

from nicegui import ui
from app.bi_dashboard_ui import BIDashboardUI

# Test the dashboard UI initialization
print("Testing BI Dashboard...")

try:
    # Initialize the dashboard
    dashboard = BIDashboardUI()
    print("✓ Dashboard initialized successfully")
    
    # Test getting welcome message
    welcome = dashboard.service.get_welcome_message()
    print(f"✓ Welcome message: {welcome.get('title', 'No title')}")
    
    # Test getting KPI metrics
    metrics = dashboard.service.get_kpi_metrics(days=30)
    print(f"✓ KPI metrics loaded: {len(metrics)} metrics")
    for metric in metrics:
        print(f"  - {metric.get('name')}: {metric.get('value')} {metric.get('unit', '')}")
    
    # Test getting revenue trend
    revenue_data = dashboard.service.get_daily_revenue_trend(days=30)
    print(f"✓ Revenue trend loaded: {len(revenue_data)} data points")
    
    print("\nAll tests passed! The application should work correctly.")
    
except Exception as e:
    print(f"✗ Error: {e}")
    import traceback
    traceback.print_exc()