"""
FastAPI backend with React Admin SimpleRestProvider compatibility.
Uses helper types and sub-routers for flexible resource management.
"""

import polars as pl
from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware
from fastapi.staticfiles import StaticFiles
from pathlib import Path

from react_admin_helpers import ResourceConfig
from routes.customers.routes import router as customers_router, set_resource_config
from routes.customers.schema import Customer

app = FastAPI(title="React Admin Compatible API")

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
    expose_headers=["X-Total-Count", "Content-Range"],
)


# Data storage
customers_df: pl.DataFrame = pl.DataFrame()

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


# Load data and configure customers resource
load_csv_data()

# Configure customers resource
customers_config = ResourceConfig(
    name="customers",
    dataframe_getter=get_customers_df,
    dataframe_setter=set_customers_df,
    model_class=Customer,
    searchable_fields=['first_name', 'last_name', 'company', 'email']
)

# Set the configuration for the customers router
set_resource_config(customers_config)

# Include the customers router
app.include_router(customers_router, prefix="/api/customers")
# Serve frontend
frontend_dist = Path("frontend/dist")
if frontend_dist.exists():
    app.mount("/", StaticFiles(directory="frontend/dist", html=True), name="frontend")


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)
