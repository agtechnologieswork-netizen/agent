<div align="center">
  <img src="logo.png" alt="app.build logo" width="150">
</div>

# app.build (agent)

> **Project Status:** The managed service has been discontinued. The Python version of this agent is no longer actively maintained but remains available for use and forking. Work report is available on [arXiv](https://arxiv.org/abs/2509.03310). Active development has moved to the Rust implementation (see `./dabgent`) focused on data applications with event sourcing architecture.

**app.build** is an open-source AI agent for generating production-ready applications with testing, linting and deployment setup from a single prompt.

## What it builds

We're currently supporting the following application types:

### tRPC CRUD Web Applications

- **Full-stack web apps** with Bun, React, Vite, Fastify, tRPC and Drizzle;
- **Automatic validation** with ESLint, TypeScript, and runtime verification;
- **Applications tested** ahead of generation with smoke tests using Playwright

### Laravel Web Applications (Alpha Version)

- **Full-stack web apps** with Laravel, React, TypeScript, Tailwind CSS, and Inertia.js;
- **Modern Laravel 12** with PHP 8+ features and strict typing;
- **Built-in authentication** with Laravel Breeze providing complete user registration, login, and profile management;
- **Production-ready features** including validation, testing infrastructure, and code style enforcement;
- **AI-powered development** that creates complete applications including models, migrations, controllers, and React components from a single prompt;

### Data-oriented Applications

- **Data apps** with Python + NiceGUI + SQLModel stack - perfect for dashboards and data visualization;
- **Automatic validation** using pytest, ruff, pyright, and runtime verification;
- **Additional packages management** with uv;

All applications support:
- **[Neon Postgres DB](https://get.neon.com/ab5)** provisioned instantly via API
- **GitHub repository** with complete source code
- **CI/CD and deployment** via the [app.build platform](https://github.com/appdotbuild/platform).

New application types are work in progress, stay tuned for updates!

## Try it

Local usage and development instructions are available in [LOCAL_SETUP.md](LOCAL_SETUP.md).

## Architecture

This agent doesn't generate entire applications at once. Instead, it breaks down app creation into small, well-scoped tasks that run in isolated sandboxes:

### tRPC Applications
1. **Database schema generation** - Creates typed database models
2. **API handler logic** - Builds validated Fastify routes
3. **Frontend components** - Generates React UI with proper typing

### Laravel Applications
1. **Database migrations & models** - Creates Laravel migrations with proper syntax and Eloquent models with PHPDoc annotations
2. **Controllers & routes** - Builds RESTful controllers with Form Request validation
3. **Inertia.js pages** - Generates React components with TypeScript interfaces
4. **Validation & testing** - Runs PHPStan, architecture tests, and feature tests

Each task is validated independently using language-specific tools (ESLint/TypeScript for JS, PHPStan for PHP), test execution, and runtime logs before being accepted.

More details on the architecture can be found in the [blog on our design decisions](https://www.app.build/blog/design-decisions).

## Custom LLM Configuration

Override default models using `backend:model` format:

```bash
# Local (Ollama and LMStudio supported)
LLM_BEST_CODING_MODEL=ollama:devstral
LLM_UNIVERSAL_MODEL=lmstudio:[host] # just lmstudio: works too

# Cloud providers
OPENROUTER_API_KEY=your-key
LLM_BEST_CODING_MODEL=openrouter:deepseek/deepseek-coder
```
Among cloud providers, we support Gemini, Anthropic, OpenAI, and OpenRouter.

**Defaults**:

```bash
LLM_BEST_CODING_MODEL=anthropic:claude-sonnet-4-20250514   # code generation
LLM_UNIVERSAL_MODEL=gemini:gemini-2.5-flash-preview-05-20  # universal model, chat with user
LLM_ULTRA_FAST_MODEL=gemini:gemini-2.5-flash-lite-preview-06-17  # commit generation etc.
LLM_VISION_MODEL=gemini:gemini-2.5-flash-lite-preview-06-17  # vision model for UI validation
```

## Repository structure

This repository contains:
- **Python agent** (`./agent/`) - Original implementation, no longer maintained but available for use and forking
- **Rust implementation** (`./edda/`) - v2, currently under in early stage (not usable), under active development

### Rust Implementation (edda)

The new Rust-based agent is being built from the ground up with:
- **Event sourcing architecture** for full auditability and replay capabilities
- **Focus on data applications** - dashboards, analytics, and data-driven tools
- **Type safety and performance** leveraging Rust's strengths
- **Early stage** - under active development

## Contributing

See [LOCAL_SETUP.md](LOCAL_SETUP.md) for development setup.

## Citation

If you use this work in your research, please cite our paper:

```bibtex
@misc{kniazev2025appbuildproductionframeworkscaling,
      title={app.build: A Production Framework for Scaling Agentic Prompt-to-App Generation with Environment Scaffolding},
      author={Evgenii Kniazev and Arseny Kravchenko and Igor Rekun and James Broadhead and Nikita Shamgunov and Pranav Sah and Pratik Nichite and Ivan Yamshchikov},
      year={2025},
      eprint={2509.03310},
      archivePrefix={arXiv},
      primaryClass={cs.AI},
      url={https://arxiv.org/abs/2509.03310}
}
```

---

Built to showcase agent-native infrastructure patterns. Fork it, remix it, use it as a reference for your own projects.
