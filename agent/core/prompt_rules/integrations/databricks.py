"""
Databricks integration rules and patterns.
"""

def get_databricks_rules() -> str:
    """Return Databricks-specific rules."""
    return """
# Databricks Integration Patterns

## Important Setup
0. **Verify Data**: Be sure to check real tables structure and data in Databricks before implementing models.

## Required Imports
1. **Core Imports**: Use the following imports for Databricks entities:
   ```python
   from app.dbrx import execute_databricks_query, DatabricksModel
   
   # Signatures:
   def execute_databricks_query(query: str) -> List[Dict[str, Any]]:
       ...
   
   class DatabricksModel(BaseModel):
       __catalog__: ClassVar[str]
       __schema__: ClassVar[str]
       __table__: ClassVar[str]
       
       @classmethod
       def table_name(cls) -> str:
           return f"{cls.__catalog__}.{cls.__schema__}.{cls.__table__}"
       
       @classmethod
       def fetch(cls, **params) -> List["DatabricksModel"]:
           raise NotImplementedError("Subclasses must implement fetch() method")
   ```

## Model Implementation
2. **DatabricksModel Usage**: Use DatabricksModel for defining models that interact with Databricks tables, and implement the fetch method to execute SQL queries and return model instances.

3. **Query Execution**: Fetch should use `execute_databricks_query` to run the SQL and convert results to model instances.

## Query Best Practices
4. **Parameterized Queries**: Use parameterized queries with proper escaping:
   ```python
   query = f\"\"\"
       SELECT city_name, country_code,
              AVG(temperature_min) as avg_min_temp,
              COUNT(*) as forecast_days
       FROM samples.accuweather.forecast_daily_calendar_imperial
       WHERE date >= (SELECT MAX(date) - INTERVAL {days} DAYS
                      FROM samples.accuweather.forecast_daily_calendar_imperial)
       GROUP BY city_name, country_code
       ORDER BY avg_max_temp DESC
   \"\"\"
   ```

5. **Result Conversion**: Convert query results to model instances in fetch methods:
   ```python
   raw_results = execute_databricks_query(query)
   return [cls(**row) for row in raw_results]
   ```

## Example Implementation
```python
class WeatherExtremes(DatabricksModel):
    __catalog__ = "samples"
    __schema__ = "accuweather"
    __table__ = "forecast_daily_calendar_imperial"
    
    coldest_temp: float
    hottest_temp: float
    highest_humidity: float
    strongest_wind: float
    locations_count: int
    date_range_days: int
    
    @classmethod
    def fetch(cls, days: int = 30, **params) -> List["WeatherExtremes"]:
        query = f\"\"\"
            SELECT MIN(temperature_min) as coldest_temp,
                   MAX(temperature_max) as hottest_temp,
                   MAX(humidity_relative_avg) as highest_humidity,
                   MAX(wind_speed_avg) as strongest_wind,
                   COUNT(DISTINCT city_name) as locations_count,
                   {days} as date_range_days
            FROM {cls.table_name()}
            WHERE date >= (SELECT MAX(date) - INTERVAL {days} DAYS FROM {cls.table_name()})
        \"\"\"
        raw_results = execute_databricks_query(query)
        result = [cls(**row) for row in raw_results]
        logger.info(f"Got {len(result)} results for WeatherExtremes")
        return result
```

## Best Practices
6. **Validation**: Always validate query results before processing
7. **Error Messages**: Use descriptive error messages for debugging
8. **Logging**: Log query execution for monitoring
9. **Performance**: Consider query performance and add appropriate limits
10. **Defaults**: Use reasonable default values for parameters in fetch methods with limits, so the default fetch does not take too long
11. **Optimization**: For quick results, fetch aggregated data from Databricks and store it in a PostgreSQL database
12. **Testing**: CRITICAL: Before creating a new DatabricksModel, make sure the query returns expected results.
"""