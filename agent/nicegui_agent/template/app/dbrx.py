
from typing import List, Dict, Any, ClassVar, Sequence, TypeVar
from databricks.sdk import WorkspaceClient
from databricks.sdk.service.sql import StatementState

from pydantic import BaseModel

T = TypeVar('T', bound='DatabricksModel')


def execute_databricks_query(query: str) -> List[Dict[str, Any]]:
    """helper function to execute SQL query via WorkspaceClient"""
    client = WorkspaceClient()

    # use warehouse to execute query
    warehouses = list(client.warehouses.list())
    if not warehouses:
        raise RuntimeError("No SQL warehouses available for query")

    warehouse_id = warehouses[0].id
    if warehouse_id is None:
        raise RuntimeError("Warehouse ID is None")

    execution = client.statement_execution.execute_statement(
        warehouse_id=warehouse_id,
        statement=query,
        wait_timeout="30s"
    )

    if execution.status is None:
        raise RuntimeError("Execution status is None")

    if execution.status.state != StatementState.SUCCEEDED:
        error_msg = f"Query failed with state: {execution.status.state}"
        if execution.status.error is not None:
            error_msg += f" - {execution.status.error.message}"
        raise RuntimeError(error_msg)

    # convert result to dictionaries
    if (execution.result is not None and
        execution.result.data_array is not None and
        execution.manifest is not None and
        execution.manifest.schema is not None and
        execution.manifest.schema.columns is not None):
        col_names = [col.name or "" for col in execution.manifest.schema.columns]
        rows = execution.result.data_array
        return [dict(zip(col_names, row)) for row in rows]

    return []

class DatabricksModel(BaseModel):
    __catalog__: ClassVar[str]
    __schema__: ClassVar[str]
    __table__: ClassVar[str]

    @classmethod
    def table_name(cls) -> str:
        return f"{cls.__catalog__}.{cls.__schema__}.{cls.__table__}"

    @classmethod
    def fetch(cls: type[T], **params) -> Sequence[T]:
        raise NotImplementedError("Subclasses must implement fetch() method")
