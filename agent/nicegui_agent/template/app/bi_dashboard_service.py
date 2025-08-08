"""
BI Dashboard Service for Bakehouse Analytics 🥐📊

Query-first implementation that fetches analytics directly from Databricks via
execute_databricks_query. Returned shapes are simple dicts the UI can render.
"""

from logging import getLogger
from typing import Dict, List, Any, Optional

from app.dbrx import execute_databricks_query
import os

logger = getLogger(__name__)


class BIDashboardService:
    @staticmethod
    def _table_and_columns() -> Dict[str, str]:
        """Resolve table and column names from env; fall back to discovery later (TODO)."""
        return {
            "table": os.getenv("SALES_TABLE_FULL_NAME", "`catalog`.`schema`.`sales_table`"),
            "date": os.getenv("SALES_DATE_COLUMN", "sale_date"),
            "datetime": os.getenv("SALES_DATETIME_COLUMN", "sale_datetime"),
            "amount": os.getenv("SALES_AMOUNT_COLUMN", "total_amount"),
            "product": os.getenv("SALES_PRODUCT_COLUMN", "product_name"),
            "customer": os.getenv("SALES_CUSTOMER_ID_COLUMN", "customer_id"),
            "franchise": os.getenv("SALES_FRANCHISE_ID_COLUMN", "franchise_id"),
            "payment": os.getenv("SALES_PAYMENT_METHOD_COLUMN", "payment_method"),
        }

    # --- Generic SQL builders for widgets (return SQL strings) ---
    @staticmethod
    def kpi_total_revenue_sql(days: int = 30) -> str:
        cfg = BIDashboardService._table_and_columns()
        return (
            "SELECT COALESCE(SUM({amount}), 0) AS value FROM {table} "
            "WHERE {date} >= current_date() - INTERVAL {days} DAYS"
        ).format(days=days, **cfg)

    @staticmethod
    def kpi_total_orders_sql(days: int = 30) -> str:
        cfg = BIDashboardService._table_and_columns()
        return (
            "SELECT COUNT(*) AS value FROM {table} "
            "WHERE {date} >= current_date() - INTERVAL {days} DAYS"
        ).format(days=days, **cfg)

    @staticmethod
    def kpi_avg_order_value_sql(days: int = 30) -> str:
        cfg = BIDashboardService._table_and_columns()
        return (
            "SELECT COALESCE(AVG({amount}), 0) AS value FROM {table} "
            "WHERE {date} >= current_date() - INTERVAL {days} DAYS"
        ).format(days=days, **cfg)

    @staticmethod
    def kpi_unique_customers_sql(days: int = 30) -> str:
        cfg = BIDashboardService._table_and_columns()
        return (
            "SELECT COUNT(DISTINCT {customer}) AS value FROM {table} "
            "WHERE {date} >= current_date() - INTERVAL {days} DAYS"
        ).format(days=days, **cfg)

    @staticmethod
    def revenue_trend_sql(days: int = 30, limit: int = 1000) -> str:
        cfg = BIDashboardService._table_and_columns()
        return (
            "SELECT CAST({date} AS DATE) AS day, SUM({amount}) AS value FROM {table} "
            "WHERE {date} >= current_date() - INTERVAL {days} DAYS "
            "GROUP BY day ORDER BY day LIMIT {limit}"
        ).format(days=days, limit=limit, **cfg)

    @staticmethod
    def top_products_sql(days: int = 30, limit: int = 10) -> str:
        cfg = BIDashboardService._table_and_columns()
        return (
            "SELECT {product} AS label, SUM({amount}) AS value FROM {table} "
            "WHERE {date} >= current_date() - INTERVAL {days} DAYS "
            "GROUP BY {product} ORDER BY value DESC LIMIT {limit}"
        ).format(days=days, limit=limit, **cfg)

    @staticmethod
    def top_locations_sql(days: int = 30, limit: int = 10) -> str:
        # Example using city as location if present; caller may modify
        cfg = BIDashboardService._table_and_columns()
        return (
            "SELECT city AS label, SUM({amount}) AS value FROM {table} "
            "WHERE {date} >= current_date() - INTERVAL {days} DAYS "
            "GROUP BY city ORDER BY value DESC LIMIT {limit}"
        ).format(days=days, limit=limit, **cfg)

    @staticmethod
    def recent_transactions_sql(limit: int = 50) -> str:
        cfg = BIDashboardService._table_and_columns()
        # Prefer datetime if available, otherwise date
        order_col = cfg.get("datetime") or cfg.get("date")
        return (
            "SELECT * FROM {table} ORDER BY {order_col} DESC LIMIT {limit}"
        ).format(order_col=order_col, limit=limit, **cfg)
    """Service class for fetching and processing BI analytics data (query-first)"""

    @staticmethod
    def get_welcome_message() -> Dict[str, str]:
        """Get welcome message with emojis for dashboard greeting"""
        return {
            "title": "Welcome to your Bakery Sales Dashboard! 🥐📈📊",
            "subtitle": "Insights await you! ✨",
            "emoji": "👋",
            "description": (
                "Discover powerful analytics for your bakery business - sales performance, "
                "customer insights, franchise metrics, and market trends all in one place"
            ),
        }

    @staticmethod
    def get_kpi_metrics(days: int = 30) -> List[Dict[str, Any]]:
        """Fetch key performance indicators with simple trend analysis.

        NOTE: Replace `catalog.schema.sales_table` with your real table.
        """
        try:
            cfg = BIDashboardService._table_and_columns()
                revenue_row = execute_databricks_query(
                    (
                        "SELECT COALESCE(SUM({amount}), 0) AS total_revenue "
                        "FROM {table} "
                        "WHERE {date} >= current_date() - INTERVAL {days} DAYS"
                    ).format(days=days, **cfg)
                )
            transactions_row = execute_databricks_query(
                """
                SELECT COUNT(*) AS total_transactions
                FROM {table}
                WHERE {date} >= current_date() - INTERVAL {days} DAYS
                """.format(days=days, **cfg)
            )
            customers_row = execute_databricks_query(
                """
                SELECT COUNT(DISTINCT {customer}) AS unique_customers
                FROM {table}
                WHERE {date} >= current_date() - INTERVAL {days} DAYS
                """.format(days=days, **cfg)
            )
            avg_order_row = execute_databricks_query(
                """
                SELECT COALESCE(AVG({amount}), 0) AS avg_transaction_value
                FROM {table}
                WHERE {date} >= current_date() - INTERVAL {days} DAYS
                """.format(days=days, **cfg)
            )

            def get_val(rows: List[Dict[str, Any]], key: str) -> float:
                return float(rows[0].get(key, 0)) if rows else 0.0

            total_revenue = get_val(revenue_row, "total_revenue")
            total_transactions = get_val(transactions_row, "total_transactions")
            unique_customers = get_val(customers_row, "unique_customers")
            avg_transaction_value = get_val(avg_order_row, "avg_transaction_value")

            return [
                {"name": "Total Revenue", "value": total_revenue, "unit": "$", "emoji": "💰", "trend": "neutral", "change_percent": None},
                {"name": "Transactions", "value": total_transactions, "unit": "", "emoji": "🛒", "trend": "neutral", "change_percent": None},
                {"name": "Avg Transaction", "value": avg_transaction_value, "unit": "$", "emoji": "💳", "trend": "neutral", "change_percent": None},
                {"name": "Active Customers", "value": unique_customers, "unit": "", "emoji": "👥", "trend": "neutral", "change_percent": None},
            ]
        except Exception as e:
            logger.error(f"Error fetching KPI metrics: {e}")
            return []

    @staticmethod
    def get_daily_revenue_trend(days: int = 30) -> List[Dict[str, Any]]:
        """Get daily revenue trend data for charts"""
        try:
            cfg = BIDashboardService._table_and_columns()
            rows = execute_databricks_query(
                """
                SELECT CAST({date} AS DATE) AS day, SUM({amount}) AS total_revenue
                FROM {table}
                WHERE {date} >= current_date() - INTERVAL {days} DAYS
                GROUP BY day
                ORDER BY day
                LIMIT 1000
                """.format(days=days, **cfg)
            )
            return [{"date": r.get("day"), "value": r.get("total_revenue", 0), "label": f"${r.get('total_revenue', 0):,.2f}"} for r in rows]
        except Exception as e:
            logger.error(f"Error fetching daily revenue trend: {e}")
            return []

    @staticmethod
    def get_product_performance_data(days: int = 30, limit: int = 10) -> Dict[str, Any]:
        """Get top performing products with sales data"""
        try:
            cfg = BIDashboardService._table_and_columns()
            products = execute_databricks_query(
                """
                SELECT {product} AS product,
                       SUM({amount}) AS total_revenue,
                       SUM(quantity) AS total_quantity,
                       COALESCE(AVG(unit_price), 0) AS avg_unit_price,
                       100.0 * SUM({amount}) / NULLIF(SUM(SUM({amount})) OVER (), 0) AS revenue_percentage
                FROM {table}
                WHERE {date} >= current_date() - INTERVAL {days} DAYS
                GROUP BY {product}
                ORDER BY total_revenue DESC
                LIMIT {limit}
                """.format(days=days, limit=limit, **cfg)
            )
            return {
                "columns": [
                    {"name": "product", "label": "Product 🥐", "field": "product"},
                    {"name": "revenue", "label": "Revenue 💰", "field": "revenue", "sortable": True},
                    {"name": "quantity", "label": "Quantity 📊", "field": "quantity", "sortable": True},
                    {"name": "avg_price", "label": "Avg Price 💵", "field": "avg_price", "sortable": True},
                    {"name": "share", "label": "Market Share 📈", "field": "share", "sortable": True},
                ],
                "rows": [
                    {
                        "product": p.get("product", ""),
                        "revenue": f"${p.get('total_revenue', 0):,.2f}",
                        "quantity": f"{int(p.get('total_quantity', 0)):,}",
                        "avg_price": f"${float(p.get('avg_unit_price', 0)):.2f}",
                        "share": f"{float(p.get('revenue_percentage', 0)):.1f}%",
                    }
                    for p in products
                ],
            }
        except Exception as e:
            logger.error(f"Error fetching product performance: {e}")
            return {"columns": [], "rows": []}

    @staticmethod
    def get_franchise_performance_data(days: int = 30, limit: int = 15) -> Dict[str, Any]:
        """Get franchise performance metrics"""
        try:
            cfg = BIDashboardService._table_and_columns()
            franchises = execute_databricks_query(
                """
                SELECT franchise_name,
                       city,
                       country,
                       SUM({amount}) AS total_revenue,
                       COUNT(*) AS transaction_count,
                       COALESCE(AVG({amount}), 0) AS avg_transaction_value,
                       COUNT(DISTINCT franchise_id) AS size
                FROM {table}
                WHERE {date} >= current_date() - INTERVAL {days} DAYS
                GROUP BY franchise_name, city, country
                ORDER BY total_revenue DESC
                LIMIT {limit}
                """.format(days=days, limit=limit, **cfg)
            )
            return {
                "columns": [
                    {"name": "name", "label": "Franchise 🏪", "field": "name"},
                    {"name": "location", "label": "Location 🌍", "field": "location"},
                    {"name": "revenue", "label": "Revenue 💰", "field": "revenue", "sortable": True},
                    {"name": "transactions", "label": "Orders 🛒", "field": "transactions", "sortable": True},
                    {"name": "avg_order", "label": "Avg Order 💳", "field": "avg_order", "sortable": True},
                    {"name": "size", "label": "Size 📏", "field": "size"},
                ],
                "rows": [
                    {
                        "name": f.get("franchise_name", ""),
                        "location": f"{f.get('city', '')}, {f.get('country', '')}",
                        "revenue": f"${f.get('total_revenue', 0):,.2f}",
                        "transactions": f"{int(f.get('transaction_count', 0)):,}",
                        "avg_order": f"${float(f.get('avg_transaction_value', 0)):.2f}",
                        "size": int(f.get('size', 0)),
                    }
                    for f in franchises
                ],
            }
        except Exception as e:
            logger.error(f"Error fetching franchise performance: {e}")
            return {"columns": [], "rows": []}

    @staticmethod
    def get_customer_segments_data(days: int = 30) -> Dict[str, Any]:
        """Get customer segmentation analysis"""
        try:
            cfg = BIDashboardService._table_and_columns()
            customers = execute_databricks_query(
                """
                SELECT customer_segment,
                       SUM({amount}) AS total_spent,
                       COUNT(*) AS transaction_count
                FROM {table}
                WHERE {date} >= current_date() - INTERVAL {days} DAYS
                GROUP BY customer_segment
                """.format(days=days, **cfg)
            )

            segments: Dict[str, Dict[str, float]] = {}
            for c in customers:
                segment = str(c.get("customer_segment", "Unknown"))
                entry = segments.setdefault(segment, {"count": 0.0, "total_spent": 0.0, "avg_transactions": 0.0})
                entry["count"] += float(c.get("transaction_count", 0))
                entry["total_spent"] += float(c.get("total_spent", 0))
                entry["avg_transactions"] += float(c.get("transaction_count", 0))

            for seg in segments.values():
                if seg["count"] > 0:
                    seg["avg_spent"] = seg["total_spent"] / seg["count"]
                    seg["avg_transactions"] = seg["avg_transactions"] / seg["count"]

            return {
                "segments": segments,
                "chart_data": {
                    "labels": list(segments.keys()),
                    "values": [data["count"] for data in segments.values()],
                    "revenue": [data["total_spent"] for data in segments.values()],
                },
            }
        except Exception as e:
            logger.error(f"Error fetching customer segments: {e}")
            return {"segments": {}, "chart_data": {"labels": [], "values": [], "revenue": []}}

    @staticmethod
    def get_payment_methods_data(days: int = 30) -> Dict[str, Any]:
        """Get payment method preferences and performance"""
        try:
            cfg = BIDashboardService._table_and_columns()
            payment_methods = execute_databricks_query(
                """
                SELECT {payment} AS payment_method,
                       COUNT(*) AS transaction_count,
                       100.0 * COUNT(*) / NULLIF(COUNT(*) OVER (), 0) AS percentage_of_transactions,
                       SUM({amount}) AS total_revenue,
                       COALESCE(AVG({amount}), 0) AS avg_transaction_value
                FROM {table}
                WHERE {date} >= current_date() - INTERVAL {days} DAYS
                GROUP BY {payment}
                ORDER BY transaction_count DESC
                LIMIT 10
                """.format(days=days, **cfg)
            )
            return {
                "chart_data": {
                    "labels": [m.get("payment_method", "") for m in payment_methods],
                    "values": [float(m.get("percentage_of_transactions", 0)) for m in payment_methods],
                    "revenue": [float(m.get("total_revenue", 0)) for m in payment_methods],
                },
                "table_data": {
                    "columns": [
                        {"name": "method", "label": "Payment Method 💳", "field": "method"},
                        {"name": "transactions", "label": "Transactions 🛒", "field": "transactions", "sortable": True},
                        {"name": "percentage", "label": "Share 📊", "field": "percentage", "sortable": True},
                        {"name": "revenue", "label": "Revenue 💰", "field": "revenue", "sortable": True},
                        {"name": "avg_value", "label": "Avg Value 💵", "field": "avg_value", "sortable": True},
                    ],
                    "rows": [
                        {
                            "method": m.get("payment_method", ""),
                            "transactions": f"{int(m.get('transaction_count', 0)):,}",
                            "percentage": f"{float(m.get('percentage_of_transactions', 0)):.1f}%",
                            "revenue": f"${float(m.get('total_revenue', 0)):,.2f}",
                            "avg_value": f"${float(m.get('avg_transaction_value', 0)):.2f}",
                        }
                        for m in payment_methods
                    ],
                },
            }
        except Exception as e:
            logger.error(f"Error fetching payment methods data: {e}")
            return {
                "chart_data": {"labels": [], "values": [], "revenue": []},
                "table_data": {"columns": [], "rows": []},
            }

    @staticmethod
    def get_geographic_performance(days: int = 30) -> Dict[str, Any]:
        """Get geographic sales performance by country"""
        try:
            cfg = BIDashboardService._table_and_columns()
            geo_data = execute_databricks_query(
                """
                SELECT country,
                       SUM({amount}) AS total_revenue,
                       COUNT(*) AS transaction_count,
                       COUNT(DISTINCT customer_id) AS unique_customers,
                       COUNT(DISTINCT franchise_id) AS unique_franchises,
                       COALESCE(AVG({amount}), 0) AS avg_transaction_value
                FROM {table}
                WHERE {date} >= current_date() - INTERVAL {days} DAYS
                GROUP BY country
                ORDER BY total_revenue DESC
                LIMIT 20
                """.format(days=days, **cfg)
            )
            return {
                "chart_data": {
                    "countries": [g.get("country", "") for g in geo_data],
                    "revenue": [float(g.get("total_revenue", 0)) for g in geo_data],
                    "customers": [int(g.get("unique_customers", 0)) for g in geo_data],
                },
                "table_data": {
                    "columns": [
                        {"name": "country", "label": "Country 🌍", "field": "country"},
                        {"name": "revenue", "label": "Revenue 💰", "field": "revenue", "sortable": True},
                        {"name": "transactions", "label": "Orders 🛒", "field": "transactions", "sortable": True},
                        {"name": "customers", "label": "Customers 👥", "field": "customers", "sortable": True},
                        {"name": "franchises", "label": "Franchises 🏪", "field": "franchises", "sortable": True},
                        {"name": "avg_order", "label": "Avg Order 💳", "field": "avg_order", "sortable": True},
                    ],
                    "rows": [
                        {
                            "country": g.get("country", ""),
                            "revenue": f"${float(g.get('total_revenue', 0)):,.2f}",
                            "transactions": f"{int(g.get('transaction_count', 0)):,}",
                            "customers": f"{int(g.get('unique_customers', 0)):,}",
                            "franchises": f"{int(g.get('unique_franchises', 0)):,}",
                            "avg_order": f"${float(g.get('avg_transaction_value', 0)):.2f}",
                        }
                        for g in geo_data
                    ],
                },
            }
        except Exception as e:
            logger.error(f"Error fetching geographic performance: {e}")
            return {
                "chart_data": {"countries": [], "revenue": [], "customers": []},
                "table_data": {"columns": [], "rows": []},
            }

    @staticmethod
    def get_hourly_sales_pattern(days: int = 30) -> Dict[str, Any]:
        """Get hourly sales patterns for operational insights"""
        try:
            cfg = BIDashboardService._table_and_columns()
            hourly_data = execute_databricks_query(
                """
                SELECT EXTRACT(HOUR FROM {datetime}) AS hour_of_day,
                       COUNT(*) AS transaction_count,
                       SUM({amount}) AS total_revenue
                FROM {table}
                WHERE {datetime} >= current_timestamp() - INTERVAL {days} DAYS
                GROUP BY hour_of_day
                ORDER BY hour_of_day
                """.format(days=days, **cfg)
            )

            return {
                "chart_data": {
                    "hours": [f"{int(h.get('hour_of_day', 0)):02d}:00" for h in hourly_data],
                    "transactions": [int(h.get('transaction_count', 0)) for h in hourly_data],
                    "revenue": [float(h.get('total_revenue', 0)) for h in hourly_data],
                },
                "peak_hour": (max(hourly_data, key=lambda x: x.get('transaction_count', 0)).get('hour_of_day', 0) if hourly_data else 0),
                "peak_revenue_hour": (max(hourly_data, key=lambda x: x.get('total_revenue', 0)).get('hour_of_day', 0) if hourly_data else 0),
            }
        except Exception as e:
            logger.error(f"Error fetching hourly sales pattern: {e}")
            return {
                "chart_data": {"hours": [], "transactions": [], "revenue": []},
                "peak_hour": 0,
                "peak_revenue_hour": 0,
            }

    @staticmethod
    def _calculate_growth_metrics(
        current_kpi: Dict[str, Any], historical_data: List[Dict[str, Any]], days: int
    ) -> Dict[str, Optional[float]]:
        """Calculate growth percentages by comparing current period with previous period"""
        if len(historical_data) < 2:
            return {}

        try:
            # Find the previous period KPI (should be the difference between total and current period)
            total_kpi = historical_data[0]  # This includes both periods

            # Calculate previous period values
            prev_revenue = float(total_kpi.get("total_revenue", 0)) - float(current_kpi.get("total_revenue", 0))
            prev_transactions = float(total_kpi.get("total_transactions", 0)) - float(current_kpi.get("total_transactions", 0))
            prev_customers = float(total_kpi.get("unique_customers", 0)) - float(current_kpi.get("unique_customers", 0))
            prev_avg_transaction = prev_revenue / prev_transactions if prev_transactions > 0 else 0

            # Get current values from dict
            curr_revenue = float(current_kpi.get("total_revenue", 0))
            curr_transactions = float(current_kpi.get("total_transactions", 0))
            curr_customers = float(current_kpi.get("unique_customers", 0))
            curr_avg_transaction = float(current_kpi.get("avg_transaction_value", 0))

            return {
                "revenue_growth": BIDashboardService._calculate_percentage_change(
                    prev_revenue, curr_revenue
                ),
                "transactions_growth": BIDashboardService._calculate_percentage_change(
                    prev_transactions, curr_transactions
                ),
                "customers_growth": BIDashboardService._calculate_percentage_change(
                    prev_customers, curr_customers
                ),
                "avg_transaction_growth": BIDashboardService._calculate_percentage_change(
                    prev_avg_transaction, curr_avg_transaction
                ),
            }
        except Exception as e:
            logger.warning(f"Error calculating growth metrics: {e}")
            return {}

    @staticmethod
    def _calculate_percentage_change(old_value: float, new_value: float) -> Optional[float]:
        """Calculate percentage change between two values"""
        if old_value == 0:
            return None
        return ((new_value - old_value) / old_value) * 100

    @staticmethod
    def _get_trend(change_percent: Optional[float]) -> str:
        """Determine trend direction based on percentage change"""
        if change_percent is None:
            return "neutral"
        if change_percent > 0:
            return "up"
        elif change_percent < 0:
            return "down"
        else:
            return "neutral"
