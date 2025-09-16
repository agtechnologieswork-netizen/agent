"""
Example models for your DataApps.

This file shows how to create Pydantic models for data validation.
Delete this file if you don't need models, or use it as a reference.
"""

from pydantic import BaseModel
from typing import Optional

# Example model - delete if not needed
class ExampleItem(BaseModel):
    id: int
    name: str
    description: Optional[str] = None

# TODO: Add your models here
# Examples:
#
# class Counter(BaseModel):
#     name: str
#     value: int = 0
#
# class User(BaseModel):
#     id: int
#     username: str
#     email: str
#
# class Product(BaseModel):
#     id: int
#     name: str
#     price: float
#     in_stock: bool = True