"""
React Admin SimpleRestProvider compatibility wrapper.
Provides a flexible middleware layer to transform between React Admin's expected format
and our internal API format.
"""

import json
from typing import Dict, List, Any, Optional, Callable, Type
from fastapi import Request, Response, HTTPException
from fastapi.responses import JSONResponse
from pydantic import BaseModel
import polars as pl


class ReactAdminParams(BaseModel):
    """Parsed React Admin query parameters"""
    sort_field: Optional[str] = None
    sort_order: Optional[str] = None
    start: int = 0
    end: Optional[int] = None
    filters: Dict[str, Any] = {}
    
    
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


class ReactAdminWrapper:
    """
    Wrapper to provide React Admin SimpleRestProvider compatibility.
    Handles query parameter transformation and response formatting.
    """
    
    def __init__(self):
        self.resources: Dict[str, ResourceConfig] = {}
    
    def register_resource(self, config: ResourceConfig):
        """Register a resource configuration"""
        self.resources[config.name] = config
    
    def parse_react_admin_params(self, request: Request) -> ReactAdminParams:
        """Parse React Admin query parameters to our internal format"""
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
    
    def transform_to_internal_params(self, params: ReactAdminParams) -> Dict[str, Any]:
        """Transform React Admin params to our internal API format"""
        internal_params = {
            '_start': params.start,
            '_end': params.end
        }
        
        if params.sort_field:
            internal_params['_sort'] = params.sort_field
            internal_params['_order'] = 'DESC' if params.sort_order == 'DESC' else 'ASC'
        
        # Handle special filter cases
        if 'id' in params.filters:
            # Handle both single ID and array of IDs
            ids = params.filters['id']
            if isinstance(ids, list):
                internal_params['id'] = ','.join(str(i) for i in ids)
            else:
                internal_params['id'] = str(ids)
        
        if 'ids' in params.filters:
            # getMany uses 'ids' filter
            internal_params['id'] = ','.join(str(i) for i in params.filters['ids'])
        
        if 'q' in params.filters:
            # General search parameter
            internal_params['q'] = params.filters['q']
        
        return internal_params
    
    def apply_filters(self, df: pl.DataFrame, config: ResourceConfig, filters: Dict[str, Any]) -> pl.DataFrame:
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
    
    async def handle_get_list(
        self, 
        resource_name: str, 
        params: ReactAdminParams
    ) -> tuple[List[Dict], int]:
        """Handle getList operation"""
        config = self.resources.get(resource_name)
        if not config:
            raise HTTPException(status_code=404, detail=f"Resource {resource_name} not found")
        
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
        df = self.apply_filters(df, config, params.filters)
        
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
    
    async def handle_get_one(self, resource_name: str, item_id: int) -> Dict:
        """Handle getOne operation"""
        config = self.resources.get(resource_name)
        if not config:
            raise HTTPException(status_code=404, detail=f"Resource {resource_name} not found")
        
        df = config.get_dataframe()
        item_df = df.filter(pl.col('id') == item_id)
        
        if item_df.is_empty():
            raise HTTPException(status_code=404, detail=f"{resource_name} not found")
        
        return item_df.to_dicts()[0]
    
    async def handle_create(self, resource_name: str, data: Dict) -> Dict:
        """Handle create operation"""
        config = self.resources.get(resource_name)
        if not config:
            raise HTTPException(status_code=404, detail=f"Resource {resource_name} not found")
        
        df = config.get_dataframe()
        
        # Generate new ID
        max_id = df['id'].max() if not df.is_empty() else 0
        new_id = (max_id + 1) if max_id is not None else 1
        
        # Remove 'id' from data if present and add our generated one
        data = {k: v for k, v in data.items() if k != 'id'}
        data['id'] = new_id
        
        # Create new row
        new_row_df = pl.DataFrame([data])
        updated_df = pl.concat([df, new_row_df])
        
        # Save back
        config.set_dataframe(updated_df)
        
        return data
    
    async def handle_update(self, resource_name: str, item_id: int, data: Dict) -> Dict:
        """Handle update operation"""
        config = self.resources.get(resource_name)
        if not config:
            raise HTTPException(status_code=404, detail=f"Resource {resource_name} not found")
        
        df = config.get_dataframe()
        
        if df.filter(pl.col('id') == item_id).is_empty():
            raise HTTPException(status_code=404, detail=f"{resource_name} not found")
        
        # Ensure ID matches
        data['id'] = item_id
        
        # Remove old row and add updated one
        df = df.filter(pl.col('id') != item_id)
        updated_row_df = pl.DataFrame([data])
        updated_df = pl.concat([df, updated_row_df]).sort('id')
        
        # Save back
        config.set_dataframe(updated_df)
        
        return data
    
    async def handle_delete(self, resource_name: str, item_id: int) -> Dict:
        """Handle delete operation"""
        config = self.resources.get(resource_name)
        if not config:
            raise HTTPException(status_code=404, detail=f"Resource {resource_name} not found")
        
        df = config.get_dataframe()
        
        item_to_delete = df.filter(pl.col('id') == item_id)
        if item_to_delete.is_empty():
            raise HTTPException(status_code=404, detail=f"{resource_name} not found")
        
        deleted_item = item_to_delete.to_dicts()[0]
        
        # Remove item
        updated_df = df.filter(pl.col('id') != item_id)
        
        # Save back
        config.set_dataframe(updated_df)
        
        return deleted_item
    
    def create_list_response(self, data: List[Dict], total: int) -> Response:
        """Create a React Admin compatible list response"""
        response = JSONResponse(content=data)
        # React Admin expects either Content-Range or X-Total-Count
        response.headers['X-Total-Count'] = str(total)
        response.headers['Access-Control-Expose-Headers'] = 'X-Total-Count'
        return response
    
    def create_item_response(self, data: Dict) -> Response:
        """Create a React Admin compatible item response"""
        return JSONResponse(content=data)