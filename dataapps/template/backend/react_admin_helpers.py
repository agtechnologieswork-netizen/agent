"""
Helper types and utilities for React Admin SimpleRestProvider compatibility.
Provides typed request/response models and helper functions.
"""

import json
from typing import Dict, List, Any, Optional, Callable, Type, Generic, TypeVar
from fastapi import Request, HTTPException, Query
from fastapi.responses import JSONResponse
from pydantic import BaseModel, Field
import polars as pl

T = TypeVar('T', bound=BaseModel)


class ReactAdminParams(BaseModel):
    """Parsed React Admin query parameters"""
    sort_field: Optional[str] = None
    sort_order: Optional[str] = None
    start: int = 0
    end: Optional[int] = None
    filters: Dict[str, Any] = {}


class ReactAdminListQuery(BaseModel):
    """React Admin list query parameters for Swagger documentation"""
    sort: Optional[str] = Field(
        None,
        description="Sort parameter as JSON string, e.g., '[\"field\",\"ASC\"]'",
        example='["first_name","ASC"]'
    )
    range: Optional[str] = Field(
        None,
        description="Range parameter as JSON string, e.g., '[0,24]'",
        example='[0,24]'
    )
    filter: Optional[str] = Field(
        None,
        description="Filter parameter as JSON string, e.g., '{\"field\":\"value\"}'",
        example='{"company":"acme"}'
    )


class ReactAdminListResponse(BaseModel, Generic[T]):
    """React Admin list response format"""
    data: List[T]
    total: int

    class Config:
        arbitrary_types_allowed = True


class ReactAdminItemResponse(BaseModel, Generic[T]):
    """React Admin single item response format"""
    data: T

    class Config:
        arbitrary_types_allowed = True


class ResourceConfig:
    """Configuration for a specific resource type"""

    def __init__(
        self,
        name: str,
        dataframe_getter: Callable[[], pl.DataFrame],
        dataframe_setter: Callable[[pl.DataFrame], None],
        model_class: Type[BaseModel],
        searchable_fields: List[str] = None
    ):
        self.name = name
        self.get_dataframe = dataframe_getter
        self.set_dataframe = dataframe_setter
        self.model_class = model_class
        self.searchable_fields = searchable_fields or []


class ReactAdminHelper:
    """Helper class for React Admin operations with proper typing"""

    @staticmethod
    def parse_query_params(request: Request) -> ReactAdminParams:
        """Parse React Admin query parameters"""
        params = ReactAdminParams()
        query_params = dict(request.query_params)

        # Parse sort parameter: sort=["field","order"]
        if 'sort' in query_params:
            try:
                sort_data = json.loads(query_params['sort'])
                if isinstance(sort_data, list) and len(sort_data) >= 2:
                    params.sort_field = sort_data[0]
                    params.sort_order = sort_data[1]
            except (json.JSONDecodeError, IndexError):
                pass

        # Parse range parameter: range=[start,end]
        if 'range' in query_params:
            try:
                range_data = json.loads(query_params['range'])
                if isinstance(range_data, list) and len(range_data) >= 2:
                    params.start = range_data[0]
                    params.end = range_data[1] + 1  # React Admin uses inclusive end
            except (json.JSONDecodeError, IndexError):
                pass

        # Parse filter parameter: filter={"field":"value",...}
        if 'filter' in query_params:
            try:
                params.filters = json.loads(query_params['filter'])
            except json.JSONDecodeError:
                params.filters = {}

        return params

    @staticmethod
    def apply_filters(
        df: pl.DataFrame,
        config: ResourceConfig,
        filters: Dict[str, Any]
    ) -> pl.DataFrame:
        """Apply filters to a Polars DataFrame"""
        filtered_df = df

        for key, value in filters.items():
            if key in ['id', 'ids', 'q']:
                continue  # These are handled separately

            if key in df.columns:
                if isinstance(value, list):
                    filtered_df = filtered_df.filter(pl.col(key).is_in(value))
                else:
                    # For string fields, use contains for partial matching
                    if df[key].dtype == pl.Utf8:
                        filtered_df = filtered_df.filter(
                            pl.col(key).str.to_lowercase().str.contains(str(value).lower())
                        )
                    else:
                        filtered_df = filtered_df.filter(pl.col(key) == value)

        return filtered_df

    @staticmethod
    def handle_get_list(
        config: ResourceConfig,
        params: ReactAdminParams
    ) -> tuple[List[Dict], int]:
        """Handle getList operation"""
        df = config.get_dataframe()

        # Apply filters
        if 'ids' in params.filters:
            ids = params.filters['ids']
            df = df.filter(pl.col('id').is_in(ids))
        elif 'id' in params.filters:
            ids = params.filters['id']
            if isinstance(ids, list):
                df = df.filter(pl.col('id').is_in(ids))
            else:
                df = df.filter(pl.col('id') == ids)

        # Apply general search
        if 'q' in params.filters and config.searchable_fields:
            search_term = params.filters['q'].lower()
            search_conditions = [
                pl.col(field).str.to_lowercase().str.contains(search_term)
                for field in config.searchable_fields
                if field in df.columns
            ]
            if search_conditions:
                df = df.filter(pl.any_horizontal(search_conditions))

        # Apply other filters
        df = ReactAdminHelper.apply_filters(df, config, params.filters)

        total_count = len(df)

        # Apply sorting
        if params.sort_field and params.sort_field in df.columns:
            df = df.sort(params.sort_field, descending=(params.sort_order == 'DESC'))

        # Apply pagination
        if params.end:
            df = df.slice(params.start, params.end - params.start)
        else:
            df = df.slice(params.start)

        return df.to_dicts(), total_count

    @staticmethod
    def handle_get_one(config: ResourceConfig, item_id: int) -> Dict:
        """Handle getOne operation"""
        df = config.get_dataframe()
        item_df = df.filter(pl.col('id') == item_id)

        if item_df.is_empty():
            raise HTTPException(status_code=404, detail=f"Item with id {item_id} not found")

        return item_df.to_dicts()[0]

    @staticmethod
    def handle_create(config: ResourceConfig, data: Dict) -> Dict:
        """Handle create operation"""
        df = config.get_dataframe()

        # Generate new ID
        max_id = df['id'].max() if not df.is_empty() else 0
        new_id = (max_id + 1) if max_id is not None else 1

        # Remove 'id' from data if present and add our generated one
        data = {k: v for k, v in data.items() if k != 'id'}
        data['id'] = new_id

        # Create new row with proper schema alignment
        if df.is_empty():
            # If the dataframe is empty, create a new one with the data
            updated_df = pl.DataFrame([data])
        else:
            # Ensure all columns from the original dataframe are present in the new row
            new_row_data = {}
            for col in df.columns:
                if col in data:
                    new_row_data[col] = data[col]
                else:
                    # Fill missing columns with None or appropriate default
                    new_row_data[col] = None
            
            new_row_df = pl.DataFrame([new_row_data])
            updated_df = pl.concat([df, new_row_df], how="vertical")

        # Save back
        config.set_dataframe(updated_df)

        return data

    @staticmethod
    def handle_update(config: ResourceConfig, item_id: int, data: Dict) -> Dict:
        """Handle update operation"""
        df = config.get_dataframe()

        # Get the existing row
        existing_row = df.filter(pl.col('id') == item_id)
        if existing_row.is_empty():
            raise HTTPException(status_code=404, detail=f"Item with id {item_id} not found")

        # Get the existing data as a dict
        existing_data = existing_row.to_dicts()[0]
        
        # Merge the existing data with the update data (update only provided fields)
        updated_data = {**existing_data, **data}
        
        # Ensure ID matches
        updated_data['id'] = item_id

        # Remove old row and add updated one with proper schema alignment
        df_without_item = df.filter(pl.col('id') != item_id)
        
        # Create new row with all columns from the original dataframe
        new_row_data = {}
        for col in df.columns:
            if col in updated_data:
                new_row_data[col] = updated_data[col]
            else:
                new_row_data[col] = None
        
        updated_row_df = pl.DataFrame([new_row_data])
        updated_df = pl.concat([df_without_item, updated_row_df], how="vertical").sort('id')

        # Save back
        config.set_dataframe(updated_df)

        return updated_data

    @staticmethod
    def handle_delete(config: ResourceConfig, item_id: int) -> Dict:
        """Handle delete operation"""
        df = config.get_dataframe()

        item_to_delete = df.filter(pl.col('id') == item_id)
        if item_to_delete.is_empty():
            raise HTTPException(status_code=404, detail=f"Item with id {item_id} not found")

        deleted_item = item_to_delete.to_dicts()[0]

        # Remove item
        updated_df = df.filter(pl.col('id') != item_id)

        # Save back
        config.set_dataframe(updated_df)

        return deleted_item

    @staticmethod
    def create_list_response(data: List[Dict], total: int) -> JSONResponse:
        """Create a React Admin compatible list response"""
        response = JSONResponse(content=data)
        response.headers['X-Total-Count'] = str(total)
        response.headers['Access-Control-Expose-Headers'] = 'X-Total-Count'
        return response

    @staticmethod
    def create_item_response(data: Dict) -> JSONResponse:
        """Create a React Admin compatible item response"""
        return JSONResponse(content=data)


def create_typed_query_params() -> tuple:
    """Create typed query parameters for FastAPI endpoints"""
    return (
        Query(None, description="Sort parameter as JSON string", example='["first_name","ASC"]'),
        Query(None, description="Range parameter as JSON string", example='[0,24]'),
        Query(None, description="Filter parameter as JSON string", example='{"company":"acme"}')
    )
