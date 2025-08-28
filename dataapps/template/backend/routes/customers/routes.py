from typing import List, Optional
from fastapi import APIRouter, Request, HTTPException, Query
from fastapi.responses import JSONResponse

from routes.customers.schema import Customer, CustomerCreate, CustomerUpdate
from react_admin_helpers import (
    ReactAdminHelper, 
    ResourceConfig,
    ReactAdminParams
)


router = APIRouter(tags=["customers"])

# This will be injected by the main app
_resource_config: Optional[ResourceConfig] = None


def set_resource_config(config: ResourceConfig):
    """Set the resource configuration for this router"""
    global _resource_config
    _resource_config = config


def get_resource_config() -> ResourceConfig:
    """Get the resource configuration, raising an error if not set"""
    if _resource_config is None:
        raise HTTPException(status_code=500, detail="Resource configuration not initialized")
    return _resource_config


@router.get(
    "/",
    response_model=List[Customer],
    summary="Get customers list",
    description="Retrieve a list of customers with optional filtering, sorting, and pagination"
)
async def get_customers(
    request: Request,
    sort: Optional[str] = Query(
        None, 
        description="Sort parameter as JSON string", 
        example='["first_name","ASC"]'
    ),
    range: Optional[str] = Query(
        None, 
        description="Range parameter as JSON string", 
        example='[0,24]'
    ),
    filter: Optional[str] = Query(
        None, 
        description="Filter parameter as JSON string", 
        example='{"company":"acme"}'
    )
):
    """Get list of customers with React Admin compatibility"""
    config = get_resource_config()
    params = ReactAdminHelper.parse_query_params(request)
    
    data, total = ReactAdminHelper.handle_get_list(config, params)
    return ReactAdminHelper.create_list_response(data, total)


@router.get(
    "/{customer_id}",
    response_model=Customer,
    summary="Get single customer",
    description="Retrieve a single customer by ID"
)
async def get_customer(customer_id: int):
    """Get a single customer by ID"""
    config = get_resource_config()
    data = ReactAdminHelper.handle_get_one(config, customer_id)
    return ReactAdminHelper.create_item_response(data)


@router.post(
    "/",
    response_model=Customer,
    status_code=201,
    summary="Create customer",
    description="Create a new customer"
)
async def create_customer(payload: CustomerCreate):
    """Create a new customer"""
    config = get_resource_config()
    data = ReactAdminHelper.handle_create(config, payload.dict())
    return ReactAdminHelper.create_item_response(data)


@router.put(
    "/{customer_id}",
    response_model=Customer,
    summary="Update customer",
    description="Update an existing customer"
)
async def update_customer(customer_id: int, payload: CustomerUpdate):
    """Update an existing customer"""
    config = get_resource_config()
    data = ReactAdminHelper.handle_update(config, customer_id, payload.dict(exclude_unset=True))
    return ReactAdminHelper.create_item_response(data)


@router.delete(
    "/{customer_id}",
    response_model=Customer,
    summary="Delete customer",
    description="Delete a customer"
)
async def delete_customer(customer_id: int):
    """Delete a customer"""
    config = get_resource_config()
    data = ReactAdminHelper.handle_delete(config, customer_id)
    return ReactAdminHelper.create_item_response(data)
