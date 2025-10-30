<div align="center">
  <img src="logo.png" alt="app.build logo" width="150">
</div>

# app.build (agent)

**app.build** is an open-source AI agent for generating production-ready applications with testing, linting and deployment setup from a single prompt. This agent relies heavily on scaffolding and extensive validation to ensure high-quality outputs.

There are two generations of this agent:

### v1 - Python implementation (⚠️ deprecated)

Original standalone agent located in `./agent/` directory (no longer actively maintained). This version can still be used and forked, it is designed to generate CRUD applications on three stacks: (TypeScript + tRPC + Drizzle + React, Python + NiceGUI + SQLModel, PHP + Laravel). The managed service has been discontinued.

See [agent/README.md](agent/README.md) for setup and some usage instructions.

Work report is available on [arXiv](https://arxiv.org/abs/2509.03310).

### v2 - Rust implementation 🦀

It is located in `./edda/` directory (under active development). The purpose of this version is to build a more robust architecture with a focus on data applications (dashboards, analytics, data-driven tools).

Unlike the Python version, it is available not only as a standalone agent but also as a MCP powering your favorite agents (like Claude Code) or being wrapped into custom agents programmatically (see `klaudbiusz/cli/codegen.py` for the example of using with Claude Agent SDK).

### MCP Installation
Try it out!

```
curl -LsSf https://raw.githubusercontent.com/appdotbuild/agent/refs/heads/main/edda/install.sh | sh
```
and attach to your favorite MCP client, e.g. Claude Code:
```
claude mcp add --transport stdio edda -- /Users/Arseny/.local/bin/edda_mcp
```
For successful usage of Databricks integration, make sure you have `DATABRICKS_HOST` and `DATABRICKS_TOKEN` environment variables set.

Got any problems during usage? Prepare a bug report:
```
edda_mcp yell [optional comment]
```

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
Supported by Neon and Databricks.

Built to showcase agent-native infrastructure patterns. Fork it, remix it, use it as a reference for your own projects.
