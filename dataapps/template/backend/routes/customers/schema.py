from typing import Optional
from pydantic import BaseModel, Field

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
