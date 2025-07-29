import anyio
import jinja2
import logging
import os
from typing import Optional, Callable, Awaitable

from core.base_node import Node
from core.workspace import Workspace
from core.actors import BaseData, FileOperationsActor
from llm.common import AsyncLLM, Message, TextRaw, Tool
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
        
        # Files configuration
        self._files_allowed_draft = [
            "server/src/schema.ts",
            "server/src/db/schema.ts", 
            "server/src/handlers/",
            "server/src/index.ts"
        ]
        self._files_allowed_frontend = [
            "client/src/App.tsx",
            "client/src/components/",
            "client/src/App.css"
        ]
        self._files_protected_frontend = [
            "client/src/components/ui/"
        ]
        self._files_relevant_draft = [
            "server/src/db/index.ts",
            "server/package.json"
        ]
        self._files_relevant_handlers = [
            "server/src/helpers/index.ts",
            "server/src/schema.ts",
            "server/src/db/schema.ts"
        ]
        self._files_relevant_frontend = [
            "server/src/schema.ts",
            "server/src/index.ts",
            "client/src/utils/trpc.ts"
        ]
        self._files_inherit_handlers = [
            "server/src/db/schema.ts",
            "server/src/schema.ts"
        ]

    async def execute(
        self,
        files: dict[str, str],
        user_prompt: str,
    ) -> Node[BaseData]:
        """Execute tRPC generation."""
        self._user_prompt = user_prompt
        
        # Update workspace with input files
        workspace = self.workspace.clone()
        for file_path, content in files.items():
            workspace.write_file(file_path, content)
        self.workspace = workspace
        
        # Determine what to generate based on existing files
        has_schema = any(f in files for f in ["server/src/schema.ts", "server/src/db/schema.ts"])
        
        if not has_schema:
            # Stage 1: Generate data model only
            await notify_stage(
                self.event_callback,
                "🎯 Starting data model generation",
                "in_progress"
            )
            
            solution = await self._generate_draft(user_prompt)
            if not solution:
                raise ValueError("Data model generation failed")
                
            await notify_stage(
                self.event_callback,
                "✅ Data model generated successfully",
                "completed"
            )
            return solution
            
        else:
            # Stage 2: Generate application based on existing schema
            await notify_stage(
                self.event_callback,
                "🚀 Starting application generation",
                "in_progress"
            )
            
            # Create a single node to collect all results
            root_workspace = self.workspace.clone().permissions(
                allowed=self._files_allowed_draft + self._files_allowed_frontend
            )
            message = Message(role="user", content=[TextRaw(f"Generate application for: {user_prompt}")])
            root_node = Node(BaseData(root_workspace, [message], {}, True))
            
            # Copy existing files to root node
            for file_path, content in files.items():
                root_node.data.files[file_path] = content
            
            # Generate implementation
            results = await self._generate_implementation(files, None)
            
            # Merge all results into root node
            for key, node in results.items():
                if node:
                    for file_path, content in node.data.files.items():
                        root_node.data.files[file_path] = content
            
            await notify_stage(
                self.event_callback,
                "✅ Application generated successfully",
                "completed"
            )
            return root_node

    async def _generate_draft(self, user_prompt: str) -> Optional[Node[BaseData]]:
        """Generate schema and type definitions."""
        self._current_context = "draft"
        
        await notify_if_callback(
            self.event_callback,
            "🎯 Generating application schema and types...",
            "draft start"
        )
        
        # Create draft workspace
        workspace = self.workspace.clone().permissions(allowed=self._files_allowed_draft)
        
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
        tx, rx = anyio.create_memory_object_stream[tuple[str, Optional[Node[BaseData]]]](100)
        
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

    async def _generate_frontend_task(
        self,
        draft_files: dict[str, str],
        feedback_data: Optional[str],
        results: dict[str, Node[BaseData]],
    ):
        """Generate frontend application."""
        self._current_context = "frontend"
        
        await notify_if_callback(
            self.event_callback,
            "🎨 Starting frontend application generation...",
            "frontend start"
        )
        
        # Create frontend workspace
        workspace = self.workspace.clone()
        for file, content in draft_files.items():
            workspace.write_file(file, content)
        workspace = workspace.permissions(
            protected=self._files_protected_frontend,
            allowed=self._files_allowed_frontend
        )
        
        # Build context
        context = []
        for path in self._files_relevant_frontend:
            content = await workspace.read_file(path)
            context.append(f"\n<file path=\"{path}\">\n{content.strip()}\n</file>\n")
        
        ui_files = await self.workspace.ls("client/src/components/ui")
        context.extend([
            f"UI components in client/src/components/ui: {ui_files}",
            f"Allowed paths and directories: {self._files_allowed_frontend}",
            f"Protected paths and directories: {self._files_protected_frontend}",
        ])
        
        # Prepare prompt
        jinja_env = jinja2.Environment()
        user_prompt_template = jinja_env.from_string(playbooks.FRONTEND_USER_PROMPT)
        user_prompt_rendered = user_prompt_template.render(
            project_context="\n".join(context),
            user_prompt=feedback_data or self._user_prompt,
        )
        
        # Create frontend node
        message = Message(role="user", content=[TextRaw(user_prompt_rendered)])
        self.frontend_node = Node(BaseData(workspace, [message], {}, True))
        
        # Search for solution
        solution = await self._search_single_node(
            self.frontend_node,
            playbooks.FRONTEND_SYSTEM_PROMPT
        )
        
        if solution:
            results["frontend"] = solution
            await notify_if_callback(
                self.event_callback,
                "✅ Frontend application generated!",
                "frontend complete"
            )

    async def _search_single_node(self, root_node: Node[BaseData], system_prompt: str) -> Optional[Node[BaseData]]:
        """Search for solution from a single node."""
        solution: Optional[Node[BaseData]] = None
        iteration = 0
        
        while solution is None:
            iteration += 1
            candidates = self._select_candidates(root_node)
            if not candidates:
                logger.info("No candidates to evaluate, search terminated")
                break
            
            logger.info(f"Iteration {iteration}: Running LLM on {len(candidates)} candidates")
            nodes = await self.run_llm(
                candidates,
                system_prompt=system_prompt,
                tools=self.tools,
                max_tokens=8192
            )
            logger.info(f"Received {len(nodes)} nodes from LLM")
            
            for i, new_node in enumerate(nodes):
                logger.info(f"Evaluating node {i+1}/{len(nodes)}")
                if await self.eval_node(new_node, self._user_prompt):
                    logger.info(f"Found solution at depth {new_node.depth}")
                    solution = new_node
                    break
        
        return solution

    def _select_candidates(self, node: Node[BaseData]) -> list[Node[BaseData]]:
        """Select candidate nodes for evaluation."""
        if node.is_leaf and node.data.should_branch:
            logger.info(f"Selecting root node {self.beam_width} times (beam search)")
            return [node] * self.beam_width
        
        all_children = node.get_all_children()
        candidates = []
        for n in all_children:
            if n.is_leaf and n.depth <= self.max_depth:
                if n.data.should_branch:
                    effective_beam_width = 1 if len(all_children) > (n.depth + 1) else self.beam_width
                    logger.info(f"Selecting candidates with effective beam width: {effective_beam_width}, current depth: {n.depth}/{self.max_depth}")
                    candidates.extend([n] * effective_beam_width)
                else:
                    candidates.append(n)
        
        logger.info(f"Selected {len(candidates)} leaf nodes for evaluation")
        return candidates

    async def eval_node(self, node: Node[BaseData], user_prompt: str) -> bool:
        """Context-aware node evaluation."""
        
        # First, process any tool uses
        tool_results, _ = await self.run_tools(node, user_prompt)
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

    async def run_checks(self, node: Node[BaseData], user_prompt: str) -> str | None:
        """Run validation checks based on context."""
        # This is handled by eval_node with context awareness
        return None

    async def _build_draft_context(self, workspace: Workspace) -> str:
        """Build context for draft generation."""
        context = []
        
        for path in self._files_relevant_draft:
            content = await workspace.read_file(path)
            context.append(f"\n<file path=\"{path}\">\n{content.strip()}\n</file>\n")
            logger.debug(f"Added {path} to context")
        
        context.extend([
            "APP_DATABASE_URL=postgres://postgres:postgres@postgres:5432/postgres",
            f"Allowed paths and directories: {self._files_allowed_draft}",
        ])
        
        return "\n".join(context)

    async def _create_handler_nodes(
        self,
        handler_files: dict[str, str],
        draft_files: dict[str, str],
        feedback_data: Optional[str]
    ):
        """Create nodes for each handler."""
        self.handler_nodes = {}
        
        # Set up workspace with inherited files
        workspace = self.workspace.clone()
        for file in self._files_inherit_handlers:
            if file in draft_files:
                workspace.write_file(file, draft_files[file])
                logger.debug(f"Copied inherited file: {file}")
        
        # Prepare jinja template
        jinja_env = jinja2.Environment()
        user_prompt_template = jinja_env.from_string(playbooks.BACKEND_HANDLER_USER_PROMPT)
        
        # Process handler files
        for file, content in handler_files.items():
            handler_name, _ = os.path.splitext(os.path.basename(file))
            logger.info(f"Processing handler: {handler_name}")
            
            # Create workspace with permissions
            allowed = [file, f"server/src/tests/{handler_name}.test.ts"]
            handler_ws = workspace.clone().permissions(allowed=allowed).write_file(file, content)
            
            # Build context with relevant files
            context = []
            for path in self._files_relevant_handlers + [file]:
                file_content = await handler_ws.read_file(path)
                context.append(f"\n<file path=\"{path}\">\n{file_content.strip()}\n</file>\n")
            
            context.append(f"Allowed paths and directories: {allowed}")
            
            # Render user prompt and create node
            user_prompt_rendered = user_prompt_template.render(
                project_context="\n".join(context),
                handler_name=handler_name,
                feedback_data=feedback_data,
            )
            
            message = Message(role="user", content=[TextRaw(user_prompt_rendered)])
            node = Node(BaseData(handler_ws, [message], {}, True))
            self.handler_nodes[handler_name] = node

    def _extract_files_from_node(self, node: Node[BaseData]) -> dict[str, str]:
        """Extract generated files from a node."""
        return node.data.files

    def _get_handler_name(self, node: Node[BaseData]) -> str:
        """Extract handler name from node's workspace."""
        for file_path in node.data.files:
            if file_path.startswith("server/src/handlers/") and file_path.endswith(".ts"):
                return os.path.splitext(os.path.basename(file_path))[0]
        return "unknown"

    @property
    def additional_tools(self) -> list[Tool]:
        """Additional tools specific to tRPC actor."""
        # Base tools from FileOperationsActor are sufficient
        return []


# Utility functions for diff_edit_actor compatibility
async def run_tsc_compile(node: Node[BaseData], event_callback=None) -> tuple[None, Optional[TextRaw]]:
    """Run TypeScript compilation on server."""
    if event_callback:
        await notify_if_callback(event_callback, "🔧 Compiling backend TypeScript...", "tsc start")
    
    result = await node.data.workspace.exec(
        ["bun", "run", "tsc", "--noEmit"],
        cwd="server"
    )
    
    if result.exit_code != 0:
        if event_callback:
            await notify_if_callback(event_callback, "❌ Backend TypeScript compilation failed", "tsc failure")
        return None, TextRaw(f"TypeScript compile errors: {result.stdout}")
    
    if event_callback:
        await notify_if_callback(event_callback, "✅ Backend TypeScript compilation passed", "tsc success")
    return None, None


async def run_tests(node: Node[BaseData], event_callback=None) -> tuple[None, Optional[TextRaw]]:
    """Run backend tests."""
    if event_callback:
        await notify_if_callback(event_callback, "🧪 Running backend tests...", "tests start")
    
    result = await node.data.workspace.exec(
        ["bun", "test"],
        cwd="server"
    )
    
    if result.exit_code != 0:
        if event_callback:
            await notify_if_callback(event_callback, "❌ Backend tests failed", "tests failure")
        return None, TextRaw(f"Test failures: {result.stdout}")
    
    if event_callback:
        await notify_if_callback(event_callback, "✅ Backend tests passed", "tests success")
    return None, None


async def run_frontend_build(node: Node[BaseData], event_callback=None) -> Optional[str]:
    """Run frontend build."""
    if event_callback:
        await notify_if_callback(event_callback, "🏗️ Building frontend application...", "build start")
    
    result = await node.data.workspace.exec(
        ["bun", "run", "build"],
        cwd="client"
    )
    
    if result.exit_code != 0:
        if event_callback:
            await notify_if_callback(event_callback, "❌ Frontend build failed", "build failure")
        return f"Frontend build errors: {result.stdout}"
    
    if event_callback:
        await notify_if_callback(event_callback, "✅ Frontend build successful", "build success")
    return None