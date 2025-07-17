"""
Python-specific rules and best practices.
"""

def get_python_rules() -> str:
    """Return Python-specific rules."""
    return """
# Python Core Rules

1. **Dependency Management**: Use `uv` for dependency management
2. **Imports**: Always use absolute imports
3. **Modern Libraries**: Prefer modern libraries (e.g. `httpx` over `requests`, `polars` over `pandas`)
4. **Modern Python**: Use modern Python features (e.g. `match` over `if`, type hints)
5. **Type Hints**: Use type hints for all functions and methods, and strictly follow them
6. **Decimal Operations**: For numeric operations with Decimal, use explicit conversion: `Decimal('0')` not `0`

# None Handling Best Practices

1. **Always Handle None**: Check if value is None if type is `Optional[T]` or `T | None`
2. **Query Results**: Check query results before using: `user = session.get(User, user_id); if user is None: return None`
3. **Guard Attributes**: Guard Optional attributes: `if item.id is not None: process(item.id)`
4. **Early Returns**: Use early returns for None checks to reduce nesting
5. **Chained Access**: For chained Optional access, check each level: `if user and user.profile and user.profile.settings:`

# Boolean Comparison Rules

1. **Truthiness**: Avoid boolean comparisons like `== True`, use truthiness instead: `if value:` not `if value == True:`
2. **Tests**: For negative assertions in tests, use `assert not validate_func()` not `assert validate_func() == False`

# Lambda Functions with Nullable Values

1. **Capture Safely**: Capture nullable values safely:
   ```python
   # WRONG: on_click=lambda: delete_user(user.id)  # user.id might be None
   # CORRECT: on_click=lambda user_id=user.id: delete_user(user_id) if user_id else None
   ```
2. **Event Handlers**: `on_click=lambda e, item_id=item.id: delete_item(item_id) if item_id else None`
3. **Alternative Pattern**: `on_click=lambda: delete_item(item.id) if item.id is not None else None`

# Error Handling

- Use try/except blocks for operations that might fail
- Log errors with context but don't expose sensitive information
- Always rethrow or handle errors explicitly
- Never use silent failures like `try: ... except: pass`
- Provide meaningful error messages to users
"""