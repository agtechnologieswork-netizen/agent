from typing import Optional
from pydantic import BaseModel, Field


class CustomerBase(BaseModel):
    """Base customer model with common fields"""
    customer_id: str = Field(description="Customer's unique identifier")
    first_name: str = Field(description="Customer's first name", max_length=100)
    last_name: str = Field(description="Customer's last name", max_length=100)
    company: str = Field(description="Company name", max_length=200)
    city: str = Field(description="City", max_length=100)
    country: str = Field(description="Country", max_length=100)
    phone_1: str = Field(description="Primary phone number", max_length=20)
    phone_2: str = Field(description="Secondary phone number", max_length=20)
    email: str = Field(description="Email address", max_length=200)
    subscription_date: str = Field(description="Subscription date (YYYY-MM-DD format)")
    website: str = Field(description="Website URL", max_length=300)


class CustomerCreate(CustomerBase):
    """Customer creation model"""
    pass


class CustomerUpdate(BaseModel):
    """Customer update model - all fields optional"""
    customer_id: Optional[str] = Field(None, description="Customer's unique identifier")
    first_name: Optional[str] = Field(None, description="Customer's first name", max_length=100)
    last_name: Optional[str] = Field(None, description="Customer's last name", max_length=100)
    company: Optional[str] = Field(None, description="Company name", max_length=200)
    city: Optional[str] = Field(None, description="City", max_length=100)
    country: Optional[str] = Field(None, description="Country", max_length=100)
    phone_1: Optional[str] = Field(None, description="Primary phone number", max_length=20)
    phone_2: Optional[str] = Field(None, description="Secondary phone number", max_length=20)
    email: Optional[str] = Field(None, description="Email address", max_length=200)
    subscription_date: Optional[str] = Field(None, description="Subscription date (YYYY-MM-DD format)")
    website: Optional[str] = Field(None, description="Website URL", max_length=300)


class Customer(CustomerBase):
    """Customer model with ID (for responses)"""
    id: int = Field(description="Internal database ID")
    
    class Config:
        from_attributes = True
