"""
FastAPI backend with React Admin SimpleRestProvider compatibility.
Uses the ReactAdminWrapper for flexible resource management.
"""

import polars as pl
from typing import Optional
from fastapi import FastAPI, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.staticfiles import StaticFiles
from pathlib import Path
from pydantic import BaseModel, Field

from react_admin_wrapper import ReactAdminWrapper, ResourceConfig

app = FastAPI(title="React Admin Compatible API")

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
    expose_headers=["X-Total-Count", "Content-Range"],
)

# Initialize the wrapper
wrapper = ReactAdminWrapper()

# Data storage
customers_df: pl.DataFrame = pl.DataFrame()

# Pydantic models
class Customer(BaseModel):
    id: Optional[int] = Field(description="Internal ID")
    customer_id: str = Field(description="Customer's ID")
    first_name: str = Field(description="Customer's first name")
    last_name: str = Field(description="Customer's last name")
    company: str = Field(description="Company name")
    city: str = Field(description="City")
    country: str = Field(description="Country")
    phone_1: str = Field(description="Primary phone number")
    phone_2: str = Field(description="Secondary phone number")
    email: str = Field(description="Email address")
    subscription_date: str = Field(description="Subscription date")
    website: str = Field(description="Website URL")


def load_csv_data():
    """Load customer data from CSV file using Polars"""
    global customers_df

    customers_df = pl.read_csv('customers-100.csv')

    customers_df = customers_df.rename({
        'Index': 'id',
        'Customer Id': 'customer_id',
        'First Name': 'first_name',
        'Last Name': 'last_name',
        'Company': 'company',
        'City': 'city',
        'Country': 'country',
        'Phone 1': 'phone_1',
        'Phone 2': 'phone_2',
        'Email': 'email',
        'Subscription Date': 'subscription_date',
        'Website': 'website'
    })

    customers_df = customers_df.with_columns(
        pl.col('id').cast(pl.Int64)
    )


def get_customers_df() -> pl.DataFrame:
    """Getter for customers dataframe"""
    return customers_df


def set_customers_df(df: pl.DataFrame):
    """Setter for customers dataframe"""
    global customers_df
    customers_df = df


# Load data and register resources
load_csv_data()

# Register customers resource with the wrapper
wrapper.register_resource(
    ResourceConfig(
        name="customers",
        dataframe_getter=get_customers_df,
        dataframe_setter=set_customers_df,
        model_class=Customer,
        searchable_fields=['first_name', 'last_name', 'company', 'email']
    )
)


# React Admin compatible endpoints
@app.get("/api/{resource}")
async def get_list(resource: str, request: Request):
    """
    Handle getList and getMany operations.
    Supports React Admin query formats:
    - sort=["field","ASC/DESC"]
    - range=[start,end]
    - filter={"field":"value"}
    """
    params = wrapper.parse_react_admin_params(request)
    data, total = await wrapper.handle_get_list(resource, params)
    return wrapper.create_list_response(data, total)


@app.get("/api/{resource}/{item_id}")
async def get_one(resource: str, item_id: int):
    """Handle getOne operation"""
    data = await wrapper.handle_get_one(resource, item_id)
    return wrapper.create_item_response(data)


@app.post("/api/{resource}")
async def create(resource: str, request: Request):
    """Handle create operation"""
    body = await request.json()
    data = await wrapper.handle_create(resource, body)
    return wrapper.create_item_response(data)


@app.put("/api/{resource}/{item_id}")
async def update(resource: str, item_id: int, request: Request):
    """Handle update operation"""
    body = await request.json()
    data = await wrapper.handle_update(resource, item_id, body)
    return wrapper.create_item_response(data)


@app.delete("/api/{resource}/{item_id}")
async def delete(resource: str, item_id: int):
    """Handle delete operation"""
    data = await wrapper.handle_delete(resource, item_id)
    return wrapper.create_item_response(data)


# Legacy endpoints for backwards compatibility
@app.get("/api/customers")
async def get_customers_legacy(
    _sort: Optional[str] = None,
    _order: Optional[str] = None,
    _start: Optional[int] = 0,
    _end: Optional[int] = None,
    q: Optional[str] = None,
    id: Optional[str] = None
):
    """Legacy endpoint - redirects to new format"""
    from fastapi import Query
    from react_admin_wrapper import ReactAdminParams

    params = ReactAdminParams(
        sort_field=_sort,
        sort_order=_order,
        start=_start,
        end=_end,
        filters={}
    )

    if id:
        params.filters['ids'] = [int(i) for i in id.split(',')]
    if q:
        params.filters['q'] = q

    data, total = await wrapper.handle_get_list('customers', params)
    return wrapper.create_list_response(data, total)


# Example of how to add a new resource (e.g., products)
# This demonstrates the flexibility of the wrapper
def create_products_resource():
    """Example function showing how to add a products resource"""

    class Product(BaseModel):
        id: Optional[int] = None
        name: str
        description: str
        price: float
        category: str
        stock: int

    # This would be your products dataframe
    products_df = pl.DataFrame()

    def get_products_df() -> pl.DataFrame:
        return products_df

    def set_products_df(df: pl.DataFrame):
        nonlocal products_df
        products_df = df

    # Register the resource
    wrapper.register_resource(
        ResourceConfig(
            name="products",
            dataframe_getter=get_products_df,
            dataframe_setter=set_products_df,
            model_class=Product,
            searchable_fields=['name', 'description', 'category']
        )
    )


# Serve frontend
frontend_dist = Path("frontend/dist")
if frontend_dist.exists():
    app.mount("/", StaticFiles(directory="frontend/dist", html=True), name="frontend")


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)
