# React Admin Frontend

This template includes a pre-configured React Admin frontend that provides scaffolding for building admin interfaces quickly.

## Quick Start

1. **Install dependencies:**
   ```bash
   npm run setup
   ```

2. **Start development servers:**
   ```bash
   npm run dev
   ```
   This starts both backend (localhost:8000) and frontend (localhost:3000)

3. **Add your resources:**
   - Backend: Add REST endpoints in `backend/main.py`
   - Frontend: Add React Admin resources in `frontend/src/App.tsx`

## Adding Resources

### Backend API
Add REST endpoints for your resources in `backend/main.py`:

```python
@app.get("/api/users")
def list_users():
    # Return list with X-Total-Count header for pagination
    return users

@app.post("/api/users") 
def create_user(user: User):
    # Create new user
    return new_user

@app.get("/api/users/{id}")
def get_user(id: int):
    # Return single user
    return user

@app.put("/api/users/{id}")
def update_user(id: int, user: User):
    # Update user
    return updated_user

@app.delete("/api/users/{id}")
def delete_user(id: int):
    # Delete user
    return {"deleted": True}
```

### Frontend Resources
Add React Admin resources in `frontend/src/App.tsx`:

```tsx
import { Admin, Resource, ListGuesser, EditGuesser, ShowGuesser } from "react-admin";

export const App = () => (
  <Admin dataProvider={dataProvider}>
    <Resource 
      name="users" 
      list={ListGuesser}
      edit={EditGuesser}
      show={ShowGuesser}
    />
  </Admin>
);
```

## React Admin Features

React Admin provides many built-in components and features:

- **Auto-generated CRUD interfaces** using `ListGuesser`, `EditGuesser`, `ShowGuesser`  
- **Filtering and searching** with built-in filter components
- **Pagination** automatically handled with REST data provider
- **Sorting** on table columns
- **Form validation** and error handling
- **Authentication** (can be added when needed)

## Architecture

- **Backend**: FastAPI at `http://localhost:8000`
  - API endpoints at `/api/{resource}`
  - Interactive docs at `/docs`
  - CORS configured for React Admin
  
- **Frontend**: React Admin at `http://localhost:3000`
  - Vite build system
  - Proxy configured to backend API
  - TypeScript support

## Next Steps

1. Define your data models in `backend/main.py`
2. Add corresponding Pydantic schemas if needed
3. Create REST endpoints following the pattern above
4. Add React Admin resources to display and manage your data

The template provides all the scaffolding - just add your business logic!