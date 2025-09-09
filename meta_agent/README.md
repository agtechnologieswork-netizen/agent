# Meta Agent Framework

A Rust-based framework for building AI agents with tree-based search, containerized execution, and template-driven optimization.

## Overview

The meta_agent is a sophisticated framework that implements an agent system using Monte Carlo Tree Search (MCTS)-style algorithms for exploring solution spaces. It provides containerized execution environments, LLM integration, and a flexible tool system for building autonomous AI agents.

## Architecture

```
meta_agent/
├── src/
│   ├── agent/           # Core agent framework
│   │   ├── actor.rs     # Agent execution with metrics
│   │   ├── mod.rs       # Traits and types (Search, Rollout, Pipeline)
│   │   ├── optimizer/   # Template-based trajectory optimization
│   │   ├── toolset.rs   # Basic tools (bash, file operations)
│   │   └── tree.rs      # Tree data structure for search
│   ├── llm.rs          # LLM client abstraction
│   ├── stacks/         # Language-specific environments
│   │   └── python/     # Python execution stack
│   └── workspace/      # Execution environments
│       ├── dagger.rs   # Containerized workspace via Dagger
│       └── mock.rs     # Mock workspace for testing
├── trajectory.json     # Sample execution trajectory
└── Cargo.toml         # Dependencies
```

## Core Components

### Agent Framework (`src/agent/`)

#### Node System
- **Node**: Represents a state in the agent's execution with conversation history and metrics
- **NodeKind**: Differentiates between active steps and completion states
- **Tree**: Manages hierarchical relationships between nodes

#### Core Traits
- **Search<T>**: Implements selection strategies for tree traversal
- **Rollout<T>**: Handles trajectory simulation and execution
- **Pipeline**: Manages command processing and event emission
- **AgentNode**: Provides workspace access for nodes

#### Tool System
- **Tool**: Base trait for tools that operate on workspaces
- **NodeTool<T>**: Specialized tools that operate on specific node types
- **AgentTool<N>**: Unified tool wrapper supporting both regular and node-specific tools
- Dynamic tool dispatch with serialized arguments and results

### LLM Integration (`src/llm.rs`)

Provides abstraction over LLM providers:
- **Completion**: Request structure with model, prompt, history, tools
- **CompletionResponse**: Standardized response format
- **LLMClientDyn**: Dynamic trait object for different LLM providers
- Support for temperature, max tokens, and provider-specific parameters

### Workspace Abstractions (`src/workspace/`)

#### Command Types
- `Bash`: Execute shell commands
- `WriteFile`/`ReadFile`: File operations
- `LsDir`: Directory listing
- `RmFile`: File removal

#### Implementations
- **MockWorkspace**: In-memory workspace for testing and development
- **DaggerWorkspace**: Containerized execution via Dagger SDK
  - Builds Docker containers from Dockerfile and context
  - Isolated execution environments
  - Persistent workspace state across operations

### Optimizer (`src/agent/optimizer/`)

Template-driven trajectory optimization:
- **Message Formatting**: Converts between internal and display formats
- **Jinja2 Templates**: For step and evaluation formatting
- **Role System**: User/Assistant message classification
- **Content Types**: Text, tool calls, and tool results
- Integration with Tera templating engine

### Stack Support (`src/stacks/`)

Language-specific execution environments:
- **Python Stack**: Docker-based Python environment setup
- Extensible for other languages and runtimes

## Usage Examples

### Basic Agent Setup

```rust
use meta_agent::agent::{Tree, Node, NodeKind};
use meta_agent::workspace::mock::MockWorkspace;

// Create initial node
let mut node = Node {
    kind: NodeKind::Step,
    history: vec![],
    workspace: Box::new(MockWorkspace::new()),
    metrics: Default::default(),
};

// Initialize tree
let tree = Tree::new(node);
```

### Tool Usage

```rust
use meta_agent::{agent::toolset::BashTool, tools_vec};

// Create tools
let tools = tools_vec![
    BashTool,
    // Add more tools...
];

// Tools are automatically dispatched based on arguments
```

### Containerized Execution

```rust
use meta_agent::workspace::dagger::DaggerRef;

let dagger = DaggerRef::new();
let workspace = dagger.workspace(
    "Dockerfile".to_string(),
    "/path/to/context".to_string()
).await?;

// Use containerized workspace for isolated execution
```

## Key Features

1. **Tree-Based Search**: MCTS-style exploration of solution spaces
2. **Containerized Execution**: Isolated environments via Dagger
3. **Dynamic Tool System**: Runtime tool discovery and dispatch
4. **Template-Driven Optimization**: Jinja2 templates for trajectory formatting
5. **Multi-Language Support**: Extensible stack system
6. **Metrics Tracking**: Token usage and execution statistics
7. **Event System**: Command/event pipeline for state management
