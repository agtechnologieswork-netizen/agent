"""
TypeScript-specific rules and best practices.
"""

def get_typescript_rules() -> str:
    """Return TypeScript-specific rules."""
    return """
# TypeScript Core Rules

1. **Type Safety**: Use strict TypeScript with proper type definitions
2. **Explicit Types**: Define explicit types for all function parameters and return values
3. **Interfaces**: Use interfaces for object shapes and contracts
4. **Null Safety**: Handle null and undefined values explicitly
5. **Async/Await**: Use async/await for asynchronous operations
6. **Modern JavaScript**: Use modern JavaScript features (ES6+)
7. **Immutability**: Prefer immutable data structures where possible

# React Best Practices

1. **Hooks**: Use React hooks properly with correct dependencies
2. **Components**: Keep components small and focused
3. **Props**: Use proper TypeScript types for props
4. **State**: Use appropriate state management (useState, useReducer, etc.)
5. **Effects**: Use useEffect correctly with proper cleanup
6. **Memoization**: Use useMemo and useCallback for performance optimization

# Database and API Rules

1. **Type Alignment**: Ensure database schemas and TypeScript types are aligned
2. **Validation**: Use runtime validation libraries (e.g., Zod) for API inputs
3. **Error Handling**: Implement proper error handling for async operations
4. **Pagination**: Implement proper pagination for large datasets
5. **Caching**: Use appropriate caching strategies for API responses

# Code Organization

1. **Imports**: Organize imports (external libraries first, then internal modules)
2. **Components**: Separate UI components from business logic
3. **Utilities**: Extract common functionality into utility functions
4. **Types**: Define types in separate files when they're shared
5. **Constants**: Use constants for magic numbers and strings
"""