# Agent Instructions for NiceGUI Dashboard Generation

## IMPORTANT: Always Create Data-Driven Widgets

When generating dashboards and widgets, you MUST:

1. **Always connect widgets to real data sources**
   - Use `DataSourceService` to discover available tables
   - Connect widgets to database tables or queries
   - Never create static/dummy data widgets when real data is available

2. **Use the Widget Tools Module**
   ```python
   from app.widget_tools import WidgetTools
   
   # Example: Create metric from database
   WidgetTools.create_metric_from_query(
       name="Active Users",
       query="SELECT COUNT(*) FROM users WHERE active = true"
   )
   ```

3. **For Databricks/Analytics Dashboards**
   - Always query actual Databricks tables when available
   - Use the bakery dataset or other sample data
   - Create charts that show real trends and patterns
   - Example queries:
     ```python
     # Sales metrics
     WidgetTools.create_metric_from_query(
         name="Total Revenue",
         query="SELECT SUM(totalPrice) FROM sales_transactions"
     )
     
     # Sales trend chart
     WidgetTools.create_chart_from_table(
         name="Daily Sales",
         table="sales_transactions",
         x_column="dateTime",
         y_column="totalPrice",
         chart_type="line"
     )
     ```

4. **Widget Configuration Best Practices**
   - Small widgets (WidgetSize.SMALL) for single metrics
   - Medium widgets (WidgetSize.MEDIUM) for charts
   - Large widgets (WidgetSize.LARGE) for tables
   - Full width (WidgetSize.FULL) for detailed views

5. **Data Source Configuration**
   Every widget should have a `data_source` configuration:
   ```python
   data_source = {
       "type": "table",  # or "query"
       "table": "sales_transactions",
       "columns": ["date", "amount"],
       "limit": 100,
       "refresh_interval": 60  # seconds
   }
   ```

6. **Auto-generate Dashboards**
   For quick setup, use:
   ```python
   from app.widget_tools import setup_data_driven_dashboard
   setup_data_driven_dashboard()
   ```

## Widget Types and Their Data Sources

### METRIC Widgets
- Display single values from aggregation queries
- Use COUNT, SUM, AVG, MAX, MIN functions
- Example: "SELECT COUNT(*) FROM orders WHERE status = 'completed'"

### CHART Widgets
- Line charts for time series data
- Bar charts for categorical comparisons
- Pie charts for proportions
- Always use real data columns for X and Y axes

### TABLE Widgets
- Display query results in tabular format
- Include pagination for large datasets
- Use ORDER BY and LIMIT for performance

### CARD Widgets
- Combine multiple data points
- Use for summary views with multiple metrics

## Example: Complete BI Dashboard

```python
from app.widget_tools import WidgetTools
from app.widget_models import WidgetSize

def create_sales_dashboard():
    # Key metrics row
    WidgetTools.create_metric_from_query(
        name="Total Revenue",
        query="SELECT SUM(totalPrice) as total FROM sales_transactions",
        icon="attach_money",
        size=WidgetSize.SMALL
    )
    
    WidgetTools.create_metric_from_query(
        name="Total Orders",
        query="SELECT COUNT(*) FROM sales_transactions",
        icon="shopping_cart",
        size=WidgetSize.SMALL
    )
    
    WidgetTools.create_metric_from_query(
        name="Avg Order Value",
        query="SELECT AVG(totalPrice) FROM sales_transactions",
        icon="trending_up",
        size=WidgetSize.SMALL
    )
    
    WidgetTools.create_metric_from_query(
        name="Unique Customers",
        query="SELECT COUNT(DISTINCT customerID) FROM sales_transactions",
        icon="people",
        size=WidgetSize.SMALL
    )
    
    # Charts row
    WidgetTools.create_chart_from_table(
        name="Sales Trend",
        table="sales_transactions",
        x_column="dateTime",
        y_column="totalPrice",
        chart_type="line",
        size=WidgetSize.MEDIUM
    )
    
    WidgetTools.create_chart_from_table(
        name="Top Products",
        table="sales_transactions",
        x_column="productName",
        y_column="quantity",
        chart_type="bar",
        size=WidgetSize.MEDIUM
    )
    
    # Recent transactions table
    WidgetTools.create_table_from_query(
        name="Recent Transactions",
        query="""
            SELECT 
                dateTime,
                customerID,
                productName,
                quantity,
                totalPrice
            FROM sales_transactions
            ORDER BY dateTime DESC
            LIMIT 20
        """,
        size=WidgetSize.FULL
    )
```

## Remember:
- **ALWAYS use real data** - Never create static demo widgets
- **Query actual tables** - Use DataSourceService.get_available_tables()
- **Connect to Databricks** when credentials are available
- **Use appropriate visualizations** for the data type
- **Set refresh intervals** for real-time updates