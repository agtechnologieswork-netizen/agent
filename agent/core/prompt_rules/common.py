"""
Common rules that apply to all languages and frameworks.
"""

def get_common_rules() -> str:
    """Return common rules for all languages and frameworks."""
    return """
# Universal Code Quality Rules

1. **Code Organization**: Keep code well-organized with clear separation of concerns
2. **Documentation**: Add comments only when necessary to explain complex logic
3. **Error Handling**: Always handle errors explicitly - never use silent failures
4. **Testing**: Write comprehensive tests for all functionality
5. **Performance**: Consider performance implications of your code
6. **Security**: Never expose or log secrets, keys, or sensitive information
7. **Readability**: Write code that is easy to understand and maintain
8. **Consistency**: Follow established patterns and conventions in the codebase
9. **Modularity**: Break down complex functionality into smaller, focused functions
10. **Validation**: Always validate inputs and handle edge cases

# Development Best Practices

- Follow the single responsibility principle
- Use meaningful variable and function names
- Avoid hardcoded values - use constants or configuration
- Write idiomatic code for the target language
- Keep functions small and focused
- Use proper dependency management
- Follow established architectural patterns
- Implement proper logging for debugging
"""