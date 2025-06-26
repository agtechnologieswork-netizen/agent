# CLAUDE.md

This file provides guidance to Claude Code when working with code in this repository.

**Refer to `PROJECT_GUIDELINES.md` for detailed project information (Overview, Structure, Workflow, Code Style, Patterns, User Lessons).**

## Common Tasks and Examples

### Docker Login Issues
If you encounter Docker login issues:

1. **ECR Authentication**: Run `./agent/scripts/docker-login.sh` to handle ECR and Docker Hub authentication
2. **AWS Credentials**: Ensure AWS credentials are configured via `aws configure` or environment variables
3. **Credential Store Issues**: Run `./agent/fix-docker-config.sh` if experiencing credential store problems

### Running Tests
- Use `docker-compose up` to start the development environment
- Run `uv run server` to start the API server

## Known Issues and Workarounds

*[Document any known issues or quirks in the codebase that Claude should be aware of]*

## Contributing Guidelines

*[Add any specific guidelines for contributing to the project]*
