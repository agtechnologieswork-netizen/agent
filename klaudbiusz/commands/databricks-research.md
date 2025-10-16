---
description: Research Databricks data using the dataresearch specialist agent
---

**IMPORTANT:** When working with Databricks data, you MUST use the `@agent-klaudbiusz:dataresearch` agent.

DO NOT use databricks_* MCP tools directly. Always delegate to dataresearch agent using the Task tool.

The dataresearch agent will:
- Explore available tables and schemas
- Execute SQL queries to understand data structure
- Fetch sample data for analysis
- Return clear findings for your implementation

Now proceed with the user's request by invoking the dataresearch agent.
