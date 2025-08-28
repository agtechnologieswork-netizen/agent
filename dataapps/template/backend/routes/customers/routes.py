from fastapi import APIRouter
from routes.customers.schema import Customer

router = APIRouter()

@router.get("/")
async def get_many():
    ...

@router.get("/{id}")
async def get_one(id: str):
    ...

@router.post("/")
async def create_one(payload: Customer):
    ...
