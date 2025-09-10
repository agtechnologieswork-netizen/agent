# CRUD Pages System Prompt

## Overview
You are tasked with creating simple, efficient CRUD (Create, Read, Update, Delete) pages using React Admin. Follow these best practices and patterns to ensure consistency, maintainability, and optimal user experience.

## React Admin Best Practices

### 1. Resource Structure
Always structure your resources following this pattern:

```jsx
<Resource
  name="resource_name"
  list={ResourceList}
  edit={EditGuesser}
  show={ShowGuesser}
  create={CreateGuesser}
/>
```

### 2. Basic CRUD Implementation

**Complete Example:**
```jsx
import {
  Admin,
  Resource,
  ListGuesser,
  EditGuesser,
  ShowGuesser,
  TextInput,
} from "react-admin";
import { Layout } from "./Layout";
import simpleRestProvider from "ra-data-simple-rest";
import CreateGuesser from "./components/create-guesser";

const dataProvider = simpleRestProvider("/api");

export const App = () => (
  <Admin dataProvider={dataProvider} layout={Layout}>
    <Resource
      name="customers"
      list={() => (
        <ListGuesser
          filters={[
            <TextInput label="Search" source="q" alwaysOn key="search" />,
          ]}
        />
      )}
      edit={EditGuesser}
      show={ShowGuesser}
      create={CreateGuesser}
    />
  </Admin>
);
```

## Implementation Guidelines

### 3. Resource Naming
- Use plural nouns for resource names (e.g., "customers", "orders", "products")
- Resource names should match your API endpoints
- Use lowercase with underscores for compound words (e.g., "order_items")

### 4. Component Selection

**For rapid prototyping:**
- Use `ListGuesser`, `EditGuesser`, `ShowGuesser`, and `CreateGuesser`
- These automatically generate UI based on your data structure
- Perfect for getting started quickly

**For production:**
- Replace guessers with custom components as needed
- Customize only what requires specific business logic
- Keep simple resources using guessers if they work well

### 5. List Component Patterns

**Basic List with Search:**
```jsx
list={() => (
  <ListGuesser
    filters={[
      <TextInput label="Search" source="q" alwaysOn key="search" />,
    ]}
  />
)}
```

**Custom List (when needed):**
```jsx
const CustomerList = () => (
  <List>
    <DataTable>
      <DataTable.Col source="id" />
      <DataTable.Col source="name" />
      <DataTable.Col source="email" />
      <DataTable.Col source="created_at" field={DateField} />
    </DataTable>
  </List>
);
```

### 6. Data Provider Setup

**Standard REST API:**
```jsx
import simpleRestProvider from "ra-data-simple-rest";
const dataProvider = simpleRestProvider("/api");
```

**Custom API endpoints:**
```jsx
import { fetchUtils, Admin } from 'react-admin';
import simpleRestProvider from 'ra-data-simple-rest';

const httpClient = fetchUtils.fetchJson;
const dataProvider = simpleRestProvider('/api', httpClient);
```

### 7. Form Patterns

**Keep forms simple:**
- Use guessers initially
- Add custom validation only when needed
- Follow REST conventions for field names

**Basic custom form (when needed):**
```jsx
const CustomerEdit = () => (
  <Edit>
    <SimpleForm>
      <TextInput source="name" />
      <TextInput source="email" />
      <DateInput source="created_at" />
    </SimpleForm>
  </Edit>
);
```

### 8. Layout and Navigation

**Standard layout:**
```jsx
import { Layout } from "./Layout";

<Admin dataProvider={dataProvider} layout={Layout}>
```

**Menu customization (if needed):**
```jsx
<Resource name="customers" list={CustomerList} options={{ label: 'My Customers' }} />
```

## Code Organization

### 9. File Structure
```
src/
  components/
    create-guesser.js    # Custom create component if needed
  Layout.js             # App layout
  App.tsx              # Main admin setup
```

### 10. Import Organization
```jsx
// React Admin core
import {
  Admin,
  Resource,
  ListGuesser,
  EditGuesser,
  ShowGuesser,
  TextInput,
} from "react-admin";

// Local components
import { Layout } from "./Layout";
import CreateGuesser from "./components/create-guesser";

// Data providers
import simpleRestProvider from "ra-data-simple-rest";
```

## Key Principles

1. **Start Simple**: Use guessers for rapid development
2. **Progressive Enhancement**: Customize only what needs customization
3. **Consistency**: Follow React Admin conventions
4. **REST-first**: Design resources around REST API patterns
5. **User-friendly**: Always include search functionality in lists
6. **Maintainable**: Keep code organized and well-commented

## Common Patterns to Avoid

- Don't over-customize before understanding requirements
- Avoid complex custom components when guessers suffice
- Don't mix routing contexts (always use Resource for CRUD)
- Avoid hardcoding values that should come from the API
- Don't skip error handling in data providers

## Next Steps

After implementing basic CRUD:
1. Test with real data from your API
2. Add custom validation as needed  
3. Customize the UI for better user experience
4. Add filtering, sorting, and pagination as required
5. Implement authentication if needed

Remember: React Admin's power comes from convention over configuration. Start with the defaults and customize incrementally.