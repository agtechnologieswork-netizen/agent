# tRPC Agent Migration Guide: Adopting Modern FileOperationsActor Architecture

## Executive Summary

### What Are We Doing?

We are refactoring the tRPC agent to adopt the modern `FileOperationsActor` base class pattern, consolidating five separate actor classes into a single, unified `TrpcActor` that extends `FileOperationsActor`.

### Why Are We Doing This?

1. **Technical Debt Reduction**
   - Current implementation has 5 separate actors with duplicated logic
   - XML-based file parsing is outdated and error-prone
   - Inconsistent patterns across different agents in the codebase

2. **Maintainability**
   - Modern agents (NiceGUI, Laravel) use a cleaner, standardized pattern
   - Tool-based file operations are more reliable and easier to debug
   - Unified error handling and validation logic

3. **Future Extensibility**
   - New features can be added as tools without modifying core logic
   - Standardized patterns make it easier for new developers to contribute
   - Better integration with future LLM capabilities (tool use is becoming standard)

4. **Performance Optimization**
   - Context-aware validation reduces unnecessary checks (e.g., handlers don't run UI tests)
   - Better error compaction using base class utilities
   - Parallel validation checks using modern async patterns

### What Will Change?

1. **Architecture**: From 5 actors to 1 unified actor
2. **File Operations**: From XML parsing to tool-based operations
3. **Validation**: From sequential to parallel, context-aware checks
4. **FSM**: From 6 states to 2 main states

### What Will Stay the Same?

1. **Parallel Handler Generation**: Critical for performance with 20+ handlers
2. **Three-Stage Flow**: Draft → Implementation → Validation
3. **Concurrent Frontend/Backend**: Both still generate in parallel
4. **All Features**: No functionality will be removed or degraded

## Overview

This document provides a comprehensive guide for migrating the tRPC agent from its current multi-actor architecture to the modern FileOperationsActor-based pattern used in NiceGUI and Laravel agents. The migration preserves all performance characteristics, especially the critical parallel generation of 20+ handlers.

## Detailed Problem Statement

### Current Pain Points

1. **Code Duplication**
   ```python
   # Same pattern repeated in DraftActor, HandlersActor, FrontendActor:
   async def eval_node(self, node):
       files_err = await run_write_files(node)  # XML parsing
       if files_err:
           node.data.messages.append(Message(role="user", content=[files_err]))
           return False
       # ... more checks
   ```

2. **XML File Parsing Complexity**
   - The `parse_files()` utility extracts files from XML tags in LLM responses
   - Error-prone and requires custom parsing logic
   - Modern LLMs work better with structured tool calls

3. **Scattered Validation Logic**
   - Each actor implements its own `eval_node` with different checks
   - No reuse of common validation patterns
   - Difficult to add new validation rules consistently

4. **Inefficient Validation**
   - Handlers run all checks even though they don't need UI validation
   - No parallel execution of independent checks
   - Each validation failure requires a full retry

5. **Complex State Machine**
   - 6 states with intermediate review steps that add no value
   - Difficult to understand flow for new developers
   - More states = more potential for bugs

### Expected Benefits

1. **Developer Experience**
   - Single file to understand instead of 5
   - Clear inheritance from base class
   - Standardized patterns matching other agents

2. **Performance Gains**
   - Skip irrelevant checks (30-50% faster validation)
   - Parallel validation checks
   - Better caching with file operation tools

3. **Reliability**
   - Tool-based operations have built-in retry logic
   - Structured error messages from tools
   - Consistent error handling across all operations

4. **Future Features**
   - Easy to add new tools (e.g., database operations)
   - Can leverage future FileOperationsActor improvements
   - Ready for multi-modal inputs (images, etc.)

## Current Architecture Analysis

### Existing Components
1. **DraftActor**: Generates schema and type definitions
2. **HandlersActor**: Generates backend handlers in parallel
3. **FrontendActor**: Generates React frontend
4. **ConcurrentActor**: Orchestrates parallel execution of HandlersActor and FrontendActor
5. **EditActor**: Already uses FileOperationsActor pattern (reference implementation)

### Key Features to Preserve
- Parallel generation of multiple handlers (critical for performance)
- Concurrent frontend and backend generation
- Context-aware validation (handlers don't need UI tests)
- Three-stage generation flow (draft → implementation → validation)

## Target Architecture

### Single TrpcActor Pattern
```python
class TrpcActor(FileOperationsActor):
    """Unified actor that manages the entire tRPC application generation."""
    
    def __init__(self, llm, vlm, workspace, beam_width=3, max_depth=30, event_callback=None):
        super().__init__(llm, workspace, beam_width, max_depth)
        self.vlm = vlm  # For playwright
        self.event_callback = event_callback
        # Sub-nodes for parallel work
        self.handler_nodes: dict[str, Node[BaseData]] = {}
        self.frontend_node: Node[BaseData] | None = None
        self.draft_node: Node[BaseData] | None = None
```

## Detailed Migration Steps

### Phase 1: Create TrpcActor Base Structure

#### 1.1 File: `trpc_agent/actors.py` (new implementation)

```python
import anyio
import jinja2
import logging
from typing import Optional, Callable, Awaitable
from anyio.streams.memory import MemoryObjectSendStream

from core.base_node import Node
from core.workspace import Workspace
from core.actors import BaseData, FileOperationsActor
from llm.common import AsyncLLM, Message, TextRaw, Tool, ToolUse, ToolUseResult
from trpc_agent import playbooks
from trpc_agent.playwright import PlaywrightRunner, drizzle_push
from core.notification_utils import notify_if_callback, notify_stage

logger = logging.getLogger(__name__)


class TrpcActor(FileOperationsActor):
    """Modern tRPC actor that generates full-stack TypeScript applications."""
    
    def __init__(
        self,
        llm: AsyncLLM,
        vlm: AsyncLLM,
        workspace: Workspace,
        beam_width: int = 3,
        max_depth: int = 30,
        event_callback: Callable[[str, str], Awaitable[None]] | None = None,
    ):
        super().__init__(llm, workspace, beam_width, max_depth)
        self.vlm = vlm
        self.event_callback = event_callback
        self.playwright = PlaywrightRunner(vlm)
        
        # Sub-nodes for parallel execution
        self.handler_nodes: dict[str, Node[BaseData]] = {}
        self.frontend_node: Optional[Node[BaseData]] = None
        self.draft_node: Optional[Node[BaseData]] = None
        
        # Context for validation
        self._current_context: str = "draft"
        self._user_prompt: str = ""
```

#### 1.2 Define File Permissions

```python
    @property
    def files_allowed(self) -> list[str]:
        """Files that can be modified."""
        return [
            "server/src/schema.ts",
            "server/src/db/schema.ts", 
            "server/src/handlers/",
            "server/src/tests/",
            "server/src/index.ts",
            "client/src/App.tsx",
            "client/src/components/",
            "client/src/App.css",
        ]
    
    @property
    def files_protected(self) -> list[str]:
        """Files that cannot be modified but can be read."""
        return [
            "Dockerfile",
            "server/src/db/index.ts",
            "client/src/utils/trpc.ts",
            "client/src/components/ui/",
        ]
    
    @property
    def files_relevant(self) -> list[str]:
        """Files to include in context."""
        return [
            "server/src/db/index.ts",
            "server/package.json",
            "client/src/utils/trpc.ts",
            "server/src/helpers/index.ts",
            "server/src/index.ts",
        ]
```

### Phase 2: Implement Three-Stage Generation

#### 2.1 Main Execute Method

```python
    async def execute(
        self,
        user_prompt: str,
        feedback_data: Optional[str] = None,
    ) -> dict[str, Node[BaseData]]:
        """Execute the three-stage tRPC application generation."""
        self._user_prompt = user_prompt
        
        await notify_stage(
            self.event_callback,
            "🚀 Starting tRPC application generation",
            "in_progress"
        )
        
        # Stage 1: Generate draft (schema + types)
        draft_solution = await self._generate_draft(user_prompt)
        if not draft_solution:
            raise ValueError("Draft generation failed")
        
        # Extract files for next stages
        draft_files = self._extract_files_from_node(draft_solution)
        
        # Stage 2: Generate implementation (handlers + frontend) in parallel
        results = await self._generate_implementation(draft_files, feedback_data)
        
        # Stage 3: Merge and return all solutions
        results["draft"] = draft_solution
        
        await notify_stage(
            self.event_callback,
            "✅ tRPC application generated successfully",
            "completed"
        )
        
        return results
```

#### 2.2 Draft Generation (Stage 1)

```python
    async def _generate_draft(self, user_prompt: str) -> Optional[Node[BaseData]]:
        """Generate schema and type definitions."""
        self._current_context = "draft"
        
        await notify_if_callback(
            self.event_callback,
            "🎯 Generating application schema and types...",
            "draft start"
        )
        
        # Create draft workspace
        workspace = self.workspace.clone().permissions(allowed=self.files_allowed)
        
        # Build context
        context = await self._build_draft_context(workspace)
        
        # Prepare prompt
        jinja_env = jinja2.Environment()
        user_prompt_template = jinja_env.from_string(playbooks.BACKEND_DRAFT_USER_PROMPT)
        user_prompt_rendered = user_prompt_template.render(
            project_context=context,
            user_prompt=user_prompt,
        )
        
        # Create root node
        message = Message(role="user", content=[TextRaw(user_prompt_rendered)])
        self.draft_node = Node(BaseData(workspace, [message], {}, True))
        
        # Search for solution
        solution = await self._search_single_node(
            self.draft_node,
            playbooks.BACKEND_DRAFT_SYSTEM_PROMPT
        )
        
        if solution:
            await notify_if_callback(
                self.event_callback,
                "✅ Schema and types generated!",
                "draft complete"
            )
        
        return solution
```

#### 2.3 Parallel Implementation Generation (Stage 2)

```python
    async def _generate_implementation(
        self,
        draft_files: dict[str, str],
        feedback_data: Optional[str] = None,
    ) -> dict[str, Node[BaseData]]:
        """Generate handlers and frontend in parallel."""
        
        results: dict[str, Node[BaseData]] = {}
        
        async with anyio.create_task_group() as tg:
            # Start frontend generation
            tg.start_soon(
                self._generate_frontend_task,
                draft_files,
                feedback_data,
                results
            )
            
            # Start parallel handler generation
            tg.start_soon(
                self._generate_handlers_parallel,
                draft_files,
                feedback_data,
                results
            )
        
        return results
    
    async def _generate_handlers_parallel(
        self,
        draft_files: dict[str, str],
        feedback_data: Optional[str],
        results: dict[str, Node[BaseData]],
    ):
        """Generate all handlers in parallel."""
        self._current_context = "handler"
        
        await notify_if_callback(
            self.event_callback,
            "🔧 Generating backend API handlers...",
            "handlers start"
        )
        
        # Create handler nodes
        handler_files = {
            path: content
            for path, content in draft_files.items()
            if path.startswith("server/src/handlers/") and path.endswith(".ts")
        }
        
        if not handler_files:
            logger.warning("No handler files found in draft")
            return
        
        # Create nodes for each handler
        await self._create_handler_nodes(handler_files, draft_files, feedback_data)
        
        # Process all handlers in parallel
        tx, rx = anyio.create_memory_object_stream[tuple[str, Optional[Node[BaseData]]]]()
        
        async def search_handler(name: str, node: Node[BaseData], tx_channel):
            await notify_if_callback(
                self.event_callback,
                f"⚡ Working on {name} handler...",
                "handler progress"
            )
            solution = await self._search_single_node(
                node,
                playbooks.BACKEND_HANDLER_SYSTEM_PROMPT
            )
            async with tx_channel:
                await tx_channel.send((name, solution))
        
        async with anyio.create_task_group() as tg:
            for name, node in self.handler_nodes.items():
                tg.start_soon(search_handler, name, node, tx.clone())
            tx.close()
            
            async with rx:
                async for (handler_name, solution) in rx:
                    if solution:
                        results[f"handler_{handler_name}"] = solution
                        logger.info(f"Handler {handler_name} completed")
        
        await notify_if_callback(
            self.event_callback,
            "✅ All backend handlers generated!",
            "handlers complete"
        )
```

### Phase 3: Context-Aware Validation

#### 3.1 Override eval_node with Context Awareness

```python
    async def eval_node(self, node: Node[BaseData], user_prompt: str) -> bool:
        """Context-aware node evaluation."""
        
        # First, process any tool uses
        tool_results = await self.process_tools(node)
        if tool_results:
            node.data.messages.append(Message(role="user", content=tool_results))
            return False
        
        # Then run context-specific validation
        match self._current_context:
            case "draft":
                return await self._eval_draft(node)
            case "handler":
                return await self._eval_handler(node)
            case "frontend":
                return await self._eval_frontend(node)
            case _:
                logger.warning(f"Unknown context: {self._current_context}")
                return True
```

#### 3.2 Context-Specific Validation Methods

```python
    async def _eval_draft(self, node: Node[BaseData]) -> bool:
        """Validate draft: TypeScript compilation + Drizzle schema."""
        errors = []
        
        async with anyio.create_task_group() as tg:
            async def check_tsc():
                result = await node.data.workspace.exec(
                    ["bun", "run", "tsc", "--noEmit"],
                    cwd="server"
                )
                if result.exit_code != 0:
                    errors.append(f"TypeScript errors:\n{result.stdout}")
            
            async def check_drizzle():
                result = await drizzle_push(
                    node.data.workspace.client,
                    node.data.workspace.ctr,
                    postgresdb=None
                )
                if result.exit_code != 0:
                    errors.append(f"Drizzle errors:\n{result.stderr}")
            
            tg.start_soon(check_tsc)
            tg.start_soon(check_drizzle)
        
        if errors:
            error_msg = await self.compact_error_message("\n".join(errors))
            node.data.messages.append(
                Message(role="user", content=[TextRaw(error_msg)])
            )
            return False
        
        return True
    
    async def _eval_handler(self, node: Node[BaseData]) -> bool:
        """Validate handler: TypeScript + tests only."""
        errors = []
        
        async with anyio.create_task_group() as tg:
            async def check_tsc():
                result = await node.data.workspace.exec(
                    ["bun", "run", "tsc", "--noEmit"],
                    cwd="server"
                )
                if result.exit_code != 0:
                    errors.append(f"TypeScript errors:\n{result.stdout}")
            
            async def check_tests():
                # Run only tests for this specific handler
                handler_name = self._get_handler_name(node)
                result = await node.data.workspace.exec(
                    ["bun", "test", f"src/tests/{handler_name}.test.ts"],
                    cwd="server"
                )
                if result.exit_code != 0:
                    errors.append(f"Test failures:\n{result.stdout}")
            
            tg.start_soon(check_tsc)
            tg.start_soon(check_tests)
        
        if errors:
            error_msg = await self.compact_error_message("\n".join(errors))
            node.data.messages.append(
                Message(role="user", content=[TextRaw(error_msg)])
            )
            return False
        
        return True
    
    async def _eval_frontend(self, node: Node[BaseData]) -> bool:
        """Validate frontend: TypeScript + build + Playwright."""
        errors = []
        
        # First, TypeScript and build checks in parallel
        async with anyio.create_task_group() as tg:
            async def check_tsc():
                result = await node.data.workspace.exec(
                    ["bun", "run", "tsc", "-p", "tsconfig.app.json", "--noEmit"],
                    cwd="client"
                )
                if result.exit_code != 0:
                    errors.append(f"TypeScript errors:\n{result.stdout}")
            
            async def check_build():
                result = await node.data.workspace.exec(
                    ["bun", "run", "build"],
                    cwd="client"
                )
                if result.exit_code != 0:
                    errors.append(f"Build errors:\n{result.stdout}")
            
            tg.start_soon(check_tsc)
            tg.start_soon(check_build)
        
        if errors:
            error_msg = await self.compact_error_message("\n".join(errors))
            node.data.messages.append(
                Message(role="user", content=[TextRaw(error_msg)])
            )
            return False
        
        # Then Playwright validation (requires built app)
        playwright_feedback = await self.playwright.evaluate(
            node,
            self._user_prompt,
            mode="client"
        )
        if playwright_feedback:
            node.data.messages.append(
                Message(role="user", content=[TextRaw(x) for x in playwright_feedback])
            )
            return False
        
        return True
```

### Phase 4: Tool Integration

#### 4.1 Process Tools Method

```python
    async def process_tools(self, node: Node[BaseData]) -> list[ToolUseResult]:
        """Process tool uses in node messages."""
        results = []
        
        for block in node.data.head().content:
            if isinstance(block, ToolUse):
                try:
                    result = await self.handle_tool(block, node)
                    results.append(result)
                except Exception as e:
                    logger.error(f"Tool {block.name} failed: {e}")
                    results.append(
                        ToolUseResult.from_tool_use(
                            block,
                            f"Tool error: {str(e)}",
                            is_error=True
                        )
                    )
        
        return results
```

#### 4.2 No Additional Tools Needed

The base FileOperationsActor tools are sufficient:
- `read_file`: Can read UI components from protected directories
- `write_file`: Write new files
- `edit_file`: Modify existing files
- `delete_file`: Remove files
- `ls`: List directory contents

### Phase 5: Update Application and FSM

#### 5.1 Simplified FSM States

```python
class FSMState(str, enum.Enum):
    GENERATION = "generation"      # Combined draft + implementation
    APPLY_FEEDBACK = "apply_feedback"
    COMPLETE = "complete"
    FAILURE = "failure"
```

#### 5.2 Updated Application Class

```python
class FSMApplication:
    """Simplified FSM application for tRPC agent."""
    
    @classmethod
    async def make_states(cls, client: dagger.Client, settings: dict[str, Any] | None = None):
        llm = get_best_coding_llm_client()
        vlm = get_vision_llm_client()
        
        workspace = await Workspace.create(
            client=client,
            base_image="oven/bun:1.2.5-alpine",
            context=client.host().directory("./trpc_agent/template"),
            setup_cmd=[["bun", "install"]],
        )
        
        event_callback = settings.get("event_callback") if settings else None
        
        # Single actor instance
        actor = TrpcActor(
            llm=llm,
            vlm=vlm,
            workspace=workspace,
            beam_width=settings.get("beam_width", 3),
            max_depth=settings.get("max_depth", 30),
            event_callback=event_callback,
        )
        
        # EditActor for feedback (already uses FileOperationsActor)
        edit_actor = EditActor(
            llm=llm,
            vlm=vlm,
            workspace=workspace,
            beam_width=settings.get("beam_width", 3),
            max_depth=settings.get("max_depth", 30),
            event_callback=event_callback,
        )
        
        # Define state transitions...
```

### Phase 6: Migration Execution Plan

#### 6.1 Step-by-Step Implementation

1. **Week 1: Core Structure**
   - Create new `TrpcActor` class extending `FileOperationsActor`
   - Implement file permission properties
   - Set up sub-node management structure

2. **Week 2: Generation Logic**
   - Port draft generation logic
   - Implement parallel handler node creation
   - Port frontend generation logic
   - Ensure proper workspace cloning and permissions

3. **Week 3: Validation System**
   - Implement context-aware `eval_node`
   - Create specific validation methods for each context
   - Port all check functions (TypeScript, Drizzle, tests, build)
   - Integrate Playwright runner

4. **Week 4: Integration & Testing**
   - Update FSM to use new actor
   - Remove old actor classes
   - Comprehensive testing with multiple handlers
   - Performance validation

#### 6.2 Testing Strategy

1. **Unit Tests**
   - Test each validation method independently
   - Verify tool processing
   - Test error compaction

2. **Integration Tests**
   - Generate app with 1 handler
   - Generate app with 10+ handlers
   - Verify parallelism is maintained
   - Test feedback application

3. **Performance Tests**
   - Measure generation time for 20+ handlers
   - Compare with current implementation
   - Ensure no regression

### Phase 7: Cleanup

#### 7.1 Files to Remove
- `actors.py` (old implementation)
- `utils.py` (XML parsing utilities)

#### 7.2 Files to Update
- `application.py` (simplified FSM)
- `playbooks.py` (ensure compatibility)

#### 7.3 Files to Keep
- `diff_edit_actor.py` (already modernized)
- `playwright.py` (validation utilities)
- `template/` (unchanged)

## Success Criteria

1. **Functionality**: All existing features work identically
2. **Performance**: No regression in generation time for 20+ handlers
3. **Code Quality**: Cleaner, more maintainable architecture
4. **Extensibility**: Easy to add new validation checks or tools
5. **Consistency**: Follows modern agent patterns

## Risks and Mitigations

### Risk 1: Performance Regression
**Mitigation**: Careful preservation of parallel patterns, extensive performance testing

### Risk 2: Validation Differences
**Mitigation**: Side-by-side testing with current implementation

### Risk 3: Complex State Management
**Mitigation**: Clear documentation of sub-node ownership and lifecycle

## Conclusion

This migration plan provides a path to modernize the tRPC agent while preserving all critical functionality and performance characteristics. The key insight is maintaining the parallel generation pattern while adopting the cleaner FileOperationsActor architecture.

The migration should be executed incrementally, with thorough testing at each phase to ensure no regressions. The end result will be a more maintainable, extensible agent that follows modern patterns while retaining the sophisticated parallel generation capabilities that make the tRPC agent performant.