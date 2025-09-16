# Frontend Integration Guide

This minimal template focuses on the backend API. For frontend integration, you have several options:

## Option 1: React Admin (Recommended for admin interfaces)

React Admin provides a ready-made admin interface that works perfectly with the FastAPI backend:

```bash
# Create React Admin app
npx create-react-app my-admin --template react-admin
cd my-admin

# Install dependencies
npm install ra-data-simple-rest

# Configure data provider to point to your FastAPI backend
# Edit src/App.js:
```

```javascript
import React from 'react';
import { Admin, Resource } from 'react-admin';
import simpleRestProvider from 'ra-data-simple-rest';

import { TaskList, TaskShow, TaskEdit, TaskCreate } from './tasks';

const dataProvider = simpleRestProvider('http://localhost:8000/api');

const App = () => (
  <Admin dataProvider={dataProvider}>
    <Resource 
      name="tasks" 
      list={TaskList} 
      show={TaskShow} 
      edit={TaskEdit} 
      create={TaskCreate} 
    />
  </Admin>
);

export default App;
```

## Option 2: Custom React App

For custom UIs, create a standard React app and use fetch or axios to call your API:

```bash
npx create-react-app my-app
cd my-app
npm install axios
```

```javascript
// Example API calls
import axios from 'axios';

const api = axios.create({
  baseURL: 'http://localhost:8000/api',
});

// Get all tasks
const tasks = await api.get('/tasks');

// Create a task
const newTask = await api.post('/tasks', {
  title: 'New Task',
  description: 'Task description',
  completed: false
});
```

## Option 3: Vue.js, Angular, or other frameworks

The FastAPI backend is framework-agnostic. Any frontend framework can consume the REST API:

### Vue.js example:
```javascript
// Using Vue 3 Composition API
import { ref, onMounted } from 'vue';

export default {
  setup() {
    const tasks = ref([]);
    
    const fetchTasks = async () => {
      const response = await fetch('http://localhost:8000/api/tasks');
      tasks.value = await response.json();
    };
    
    onMounted(fetchTasks);
    
    return { tasks };
  }
};
```

### Angular example:
```typescript
// tasks.service.ts
import { Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';

@Injectable()
export class TasksService {
  private apiUrl = 'http://localhost:8000/api';
  
  constructor(private http: HttpClient) {}
  
  getTasks() {
    return this.http.get<Task[]>(`${this.apiUrl}/tasks`);
  }
}
```

## API Endpoints

Your FastAPI backend provides these endpoints:

- `GET /api/tasks` - List all tasks (with pagination, sorting, filtering)
- `GET /api/tasks/{id}` - Get a specific task
- `POST /api/tasks` - Create a new task
- `PUT /api/tasks/{id}` - Update a task
- `DELETE /api/tasks/{id}` - Delete a task
- `GET /health` - Health check

## CORS Configuration

The backend is already configured with CORS to allow frontend requests from any origin during development. For production, update the CORS settings in `backend/main.py`:

```python
app.add_middleware(
    CORSMiddleware,
    allow_origins=["https://yourdomain.com"],  # Restrict to your domain
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
    expose_headers=["X-Total-Count", "Content-Range"],
)
```

## Development Setup

1. Start the backend:
   ```bash
   cd backend
   uv run uvicorn main:app --reload --host 0.0.0.0 --port 8000
   ```

2. Start your frontend (example for React):
   ```bash
   cd frontend/my-app
   npm start
   ```

3. Your API will be available at `http://localhost:8000`
4. API documentation at `http://localhost:8000/docs`