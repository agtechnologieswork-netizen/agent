from dataclasses import dataclass
from typing import List, Optional
import polars as pl
from databricks.sdk import WorkspaceClient
from databricks.sdk.service.sql import StatementState

from log import get_logger

logger = get_logger(__name__)


@dataclass
class TableMetadata:
    catalog: str
    schema: str
    name: str
    full_name: str
    table_type: str
    owner: Optional[str] = None
    comment: Optional[str] = None
    storage_location: Optional[str] = None
    data_source_format: Optional[str] = None
    created_at: Optional[str] = None
    updated_at: Optional[str] = None


@dataclass
class ColumnMetadata:
    name: str
    data_type: str
    comment: Optional[str] = None
    nullable: Optional[bool] = None
    position: Optional[int] = None


@dataclass
class TableDetails:
    metadata: TableMetadata
    columns: List[ColumnMetadata]
    sample_data: Optional[pl.DataFrame] = None
    row_count: Optional[int] = None
    size_bytes: Optional[int] = None


class DatabricksClient:

    def __init__(self, workspace_client: Optional[WorkspaceClient] = None):
        self.client = workspace_client or WorkspaceClient()
        logger.info("Initialized Databricks client")

    def list_tables(
        self,
        catalog: str = "*",
        schema: str = "*",
        exclude_inaccessible: bool = True
    ) -> List[TableMetadata]:
        logger.info(f"Listing tables: catalog={catalog}, schema={schema}, exclude_inaccessible={exclude_inaccessible}")

        tables = []

        # Get list of catalogs
        if catalog == "*":
            catalogs = list(self.client.catalogs.list())
            catalog_names = [c.name for c in catalogs]
            logger.debug(f"Found {len(catalog_names)} catalogs")
        else:
            catalog_names = [catalog]

        # Iterate through catalogs
        for catalog_name in catalog_names:
            # Get list of schemas
            if schema == "*":
                schemas = list(self.client.schemas.list(catalog_name=catalog_name))
                schema_names = [s.name for s in schemas]
                logger.debug(f"Found {len(schema_names)} schemas in catalog {catalog_name}")
            else:
                schema_names = [schema]

            # Iterate through schemas
            for schema_name in schema_names:
                # List tables in schema
                table_list = list(self.client.tables.list(
                    catalog_name=catalog_name,
                    schema_name=schema_name
                ))
                logger.debug(f"Found {len(table_list)} tables in {catalog_name}.{schema_name}")

                for table in table_list:
                    # Skip if exclude_inaccessible is True and we can't access
                    if exclude_inaccessible and not self._has_table_access(table.full_name):
                        logger.debug(f"Skipping inaccessible table: {table.full_name}")
                        continue

                    tables.append(TableMetadata(
                        catalog=catalog_name,
                        schema=schema_name,
                        name=table.name,
                        full_name=table.full_name,
                        table_type=table.table_type.value if table.table_type else "UNKNOWN",
                        owner=table.owner,
                        comment=table.comment,
                        storage_location=table.storage_location,
                        data_source_format=table.data_source_format.value if table.data_source_format else None,
                        created_at=str(table.created_at) if table.created_at else None,
                        updated_at=str(table.updated_at) if table.updated_at else None
                    ))

        logger.info(f"Found {len(tables)} accessible tables")
        return tables

    def get_table_details(self, table_full_name: str, sample_size: int = 10) -> TableDetails:
        logger.info(f"Getting details for table: {table_full_name}")

        # Get table metadata
        table = self.client.tables.get(table_full_name)

        # Parse table metadata
        parts = table_full_name.split(".")
        if len(parts) != 3:
            raise ValueError(f"Invalid table name format: {table_full_name}. Expected catalog.schema.table")

        metadata = TableMetadata(
            catalog=parts[0],
            schema=parts[1],
            name=parts[2],
            full_name=table_full_name,
            table_type=table.table_type.value if table.table_type else "UNKNOWN",
            owner=table.owner,
            comment=table.comment,
            storage_location=table.storage_location,
            data_source_format=table.data_source_format.value if table.data_source_format else None,
            created_at=str(table.created_at) if table.created_at else None,
            updated_at=str(table.updated_at) if table.updated_at else None
        )

        # Parse columns
        columns = []
        if table.columns:
            for i, col in enumerate(table.columns):
                columns.append(ColumnMetadata(
                    name=col.name,
                    data_type=col.type_name,
                    comment=col.comment,
                    nullable=col.nullable,
                    position=i
                ))

        # Get sample data
        sample_data = None
        row_count = None

        # Execute sample query
        sample_query = f"SELECT * FROM {table_full_name} LIMIT {sample_size}"
        logger.debug(f"Executing sample query: {sample_query}")

        # Use warehouse to execute query
        warehouses = list(self.client.warehouses.list())
        if not warehouses:
            raise RuntimeError("No SQL warehouses available for sample query")

        warehouse_id = warehouses[0].id
        execution = self.client.statement_execution.execute_statement(
            warehouse_id=warehouse_id,
            statement=sample_query,
            wait_timeout="30s"
        )

        if execution.status.state != StatementState.SUCCEEDED:
            raise RuntimeError(f"Sample query failed with state: {execution.status.state}")

        # Convert result to polars DataFrame
        if execution.result and execution.result.data_array:
            col_names = [col.name for col in execution.manifest.schema.columns]
            sample_data = pl.DataFrame(execution.result.data_array, schema=col_names, orient="row")
            logger.debug(f"Retrieved {len(sample_data)} sample rows")

        # Get row count
        count_query = f"SELECT COUNT(*) as count FROM {table_full_name}"
        execution = self.client.statement_execution.execute_statement(
            warehouse_id=warehouse_id,
            statement=count_query,
            wait_timeout="30s"
        )

        if execution.status.state != StatementState.SUCCEEDED:
            raise RuntimeError(f"Count query failed with state: {execution.status.state}")

        if execution.result and execution.result.data_array:
            row_count = int(execution.result.data_array[0][0])
            logger.debug(f"Table has {row_count} rows")
        else:
            raise RuntimeError("Count query returned no results")

        return TableDetails(
            metadata=metadata,
            columns=columns,
            sample_data=sample_data,
            row_count=row_count
        )

    def _has_table_access(self, table_full_name: str) -> bool:
        try:
            # Try to get table info - this will fail if no access
            self.client.tables.get(table_full_name)
            return True
        except Exception:
            return False

    def execute_query(self, query: str, timeout: str = "60s") -> pl.DataFrame:
        """Execute a SELECT query and return results as a polars DataFrame.
        
        Args:
            query: SQL query to execute (must be a SELECT statement)
            timeout: Query execution timeout (default: 60s)
            
        Returns:
            polars DataFrame with query results
            
        Raises:
            ValueError: If query is not a SELECT statement
            RuntimeError: If no warehouses available or query execution fails
        """
        logger.info(f"Executing query: {query[:100]}...")
        
        # validate it's a SELECT query for safety
        query_upper = query.strip().upper()
        if not query_upper.startswith("SELECT"):
            raise ValueError("Only SELECT queries are allowed for safety reasons")
        
        # get available warehouses
        warehouses = list(self.client.warehouses.list())
        if not warehouses:
            raise RuntimeError("No SQL warehouses available for query execution")
        
        warehouse_id = warehouses[0].id
        logger.debug(f"Using warehouse: {warehouse_id}")
        
        # execute the query
        execution = self.client.statement_execution.execute_statement(
            warehouse_id=warehouse_id,
            statement=query,
            wait_timeout=timeout
        )
        
        if execution.status.state != StatementState.SUCCEEDED:
            error_msg = f"Query failed with state: {execution.status.state}"
            if execution.status.error:
                error_msg += f" - {execution.status.error.message}"
            raise RuntimeError(error_msg)
        
        # convert result to polars DataFrame
        if execution.result and execution.result.data_array:
            col_names = [col.name for col in execution.manifest.schema.columns]
            df = pl.DataFrame(execution.result.data_array, schema=col_names, orient="row")
            logger.info(f"Query returned {len(df)} rows with {len(df.columns)} columns")
            return df
        else:
            # return empty DataFrame if no results
            logger.info("Query returned no results")
            return pl.DataFrame()

