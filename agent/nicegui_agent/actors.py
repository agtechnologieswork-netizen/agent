import re
import jinja2
import logging
import dataclasses
import anyio
from core.base_node import Node
from core.workspace import Workspace
from core.actors import BaseData, BaseActor, LLMActor
from llm.common import AsyncLLM, Message, TextRaw, Tool, ToolUse, ToolUseResult
from nicegui_agent import playbooks

logger = logging.getLogger(__name__)


class NiceguiActor(BaseActor, LLMActor):
    root: Node[BaseData] | None = None

    def __init__(
        self,
        llm: AsyncLLM,
        workspace: Workspace,
        beam_width: int = 3,
        max_depth: int = 30,
        system_prompt: str = playbooks.APPLICATION_SYSTEM_PROMPT,
    ):
        self.llm = llm
        self.workspace = workspace
        self.beam_width = beam_width
        self.max_depth = max_depth
        self.system_prompt = system_prompt
        self.root = None
        logger.info(
            f"Initialized {self.__class__.__name__} with beam_width={beam_width}, max_depth={max_depth}"
        )

    async def execute(
        self,
        files: dict[str, str],
        user_prompt: str,
    ) -> Node[BaseData]:
        workspace = self.workspace.clone()
        logger.info(
            f"Start {self.__class__.__name__} execution with files: {files.keys()}"
        )
        for file_path, content in files.items():
            workspace.write_file(file_path, content)
        workspace.permissions(
            protected=self.files_protected, allowed=self.files_allowed
        )

        jinja_env = jinja2.Environment()
        user_prompt_template = jinja_env.from_string(playbooks.USER_PROMPT)
        repo_files = await self.get_repo_files(workspace, files)
        project_context = "\n".join(
            [
                "Project files:",
                *repo_files,
                "Writeable files and directories:",
                *self.files_allowed,
            ]
        )
        user_prompt_rendered = user_prompt_template.render(
            project_context=project_context,
            user_prompt=user_prompt,
        )
        message = Message(role="user", content=[TextRaw(user_prompt_rendered)])
        self.root = Node(BaseData(workspace, [message], {}))

        solution: Node[BaseData] | None = None
        iteration = 0
        while solution is None:
            iteration += 1
            candidates = self.select(self.root)
            if not candidates:
                logger.info("No candidates to evaluate, search terminated")
                break

            logger.info(
                f"Iteration {iteration}: Running LLM on {len(candidates)} candidates"
            )
            nodes = await self.run_llm(
                candidates,
                system_prompt=self.system_prompt,
                tools=self.tools,
                max_tokens=8192,
            )
            logger.info(f"Received {len(nodes)} nodes from LLM")

            for i, new_node in enumerate(nodes):
                logger.info(f"Evaluating node {i + 1}/{len(nodes)}")
                if await self.eval_node(new_node, user_prompt):
                    logger.info(f"Found solution at depth {new_node.depth}")
                    solution = new_node
                    break
        if solution is None:
            logger.error(f"{self.__class__.__name__} failed to find a solution")
            raise ValueError("No solutions found")
        return solution

    def select(self, node: Node[BaseData]) -> list[Node[BaseData]]:
        candidates = []
        all_children = node.get_all_children()
        effective_beam_width = (
            1 if len(all_children) >= self.beam_width else self.beam_width
        )
        logger.info(
            f"Selecting candidates with effective beam width: {effective_beam_width}, total children: {len(all_children)}"
        )
        for n in all_children:
            if n.is_leaf and n.depth <= self.max_depth:
                if n.data.should_branch:
                    candidates.extend([n] * effective_beam_width)
                else:
                    candidates.append(n)
        logger.info(f"Selected {len(candidates)} leaf nodes for evaluation")
        return candidates

    @property
    def tools(self) -> list[Tool]:
        return [
            {
                "name": "read_file",
                "description": "Read file content",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                    },
                    "required": ["path"],
                },
            },
            {
                "name": "write_file",
                "description": "Write content to a file",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "content": {"type": "string"},
                    },
                    "required": ["path", "content"],
                },
            },
            {
                "name": "edit_file",
                "description": "Edit a file by searching and replacing text",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "search": {"type": "string"},
                        "replace": {"type": "string"},
                    },
                    "required": ["path", "search", "replace"],
                },
            },
            {
                "name": "delete_file",
                "description": "Delete a file",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                    },
                    "required": ["path"],
                },
            },
            {
                "name": "uv_add",
                "description": "Install additional packages",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "packages": {"type": "array", "items": {"type": "string"}},
                    },
                    "required": ["packages"],
                },
            },
            {
                "name": "complete",
                "description": "Mark the task as complete. This will run tests and type checks to ensure the changes are correct.",
                "input_schema": {
                    "type": "object",
                    "properties": {},
                },
            },
        ]

    async def run_tools(
        self, node: Node[BaseData], user_prompt: str
    ) -> tuple[list[ToolUseResult], bool]:
        logger.info(f"Running tools for node {node._id}")
        result, is_completed = [], False
        for block in node.data.head().content:
            if not isinstance(block, ToolUse):
                continue
            try:
                logger.info(f"Running tool {block.name} with input {block.input}")

                match block.name:
                    case "read_file":
                        tool_content = await node.data.workspace.read_file(
                            block.input["path"]
                        )  # pyright: ignore[reportIndexIssue]
                        result.append(ToolUseResult.from_tool_use(block, tool_content))
                    case "write_file":
                        path = block.input["path"]  # pyright: ignore[reportIndexIssue]
                        content = block.input["content"]  # pyright: ignore[reportIndexIssue]
                        try:
                            node.data.workspace.write_file(path, content)
                            node.data.files.update({path: content})
                            result.append(ToolUseResult.from_tool_use(block, "success"))
                            logger.debug(f"Written file: {path}")
                        except FileNotFoundError as e:
                            error_msg = f"Directory not found for file '{path}': {str(e)}"
                            logger.info(f"File not found error writing file {path}: {str(e)}")
                            result.append(ToolUseResult.from_tool_use(block, error_msg, is_error=True))
                        except PermissionError as e:
                            error_msg = f"Permission denied writing file '{path}': {str(e)}"
                            logger.info(f"Permission error writing file {path}: {str(e)}")
                            result.append(ToolUseResult.from_tool_use(block, error_msg, is_error=True))
                        except ValueError as e:
                            error_msg = str(e)
                            logger.info(f"Value error writing file {path}: {error_msg}")
                            result.append(ToolUseResult.from_tool_use(block, error_msg, is_error=True))
                    case "edit_file":
                        path = block.input["path"]  # pyright: ignore[reportIndexIssue]
                        search = block.input["search"]  # pyright: ignore[reportIndexIssue]
                        replace = block.input["replace"]  # pyright: ignore[reportIndexIssue]
                        try:
                            original = await node.data.workspace.read_file(path)
                            match original.count(search):
                                case 0:
                                    raise ValueError(
                                        f"Search text not found in file '{path}'. Search:\n{search}"
                                    )
                                case 1:
                                    new_content = original.replace(search, replace)
                                    node.data.workspace.write_file(path, new_content)
                                    node.data.files.update({path: new_content})
                                    result.append(ToolUseResult.from_tool_use(block, "success"))
                                    logger.debug(f"Applied edit to file: {path}")
                                case num_hits:
                                    raise ValueError(
                                        f"Search text found {num_hits} times in file '{path}' (expected exactly 1). Search:\n{search}"
                                    )
                        except FileNotFoundError as e:
                            error_msg = f"File '{path}' not found for editing: {str(e)}"
                            logger.info(f"File not found error editing file {path}: {str(e)}")
                            result.append(ToolUseResult.from_tool_use(block, error_msg, is_error=True))
                        except PermissionError as e:
                            error_msg = f"Permission denied editing file '{path}': {str(e)}"
                            logger.info(f"Permission error editing file {path}: {str(e)}")
                            result.append(ToolUseResult.from_tool_use(block, error_msg, is_error=True))
                        except ValueError as e:
                            error_msg = str(e)
                            logger.info(f"Value error editing file {path}: {error_msg}")
                            result.append(ToolUseResult.from_tool_use(block, error_msg, is_error=True))
                    case "delete_file":
                        node.data.workspace.rm(block.input["path"])  # pyright: ignore[reportIndexIssue]
                        node.data.files.update({block.input["path"]: None})  # pyright: ignore[reportIndexIssue]
                        result.append(ToolUseResult.from_tool_use(block, "success"))
                    case "uv_add":
                        packages = block.input["packages"]  # pyright: ignore[reportIndexIssue]
                        exec_res = await node.data.workspace.exec_mut(
                            ["uv", "add", " ".join(packages)]
                        )
                        if exec_res.exit_code != 0:
                            result.append(
                                ToolUseResult.from_tool_use(
                                    block,
                                    f"Failed to add packages: {exec_res.stderr}",
                                    is_error=True,
                                )
                            )
                        else:
                            node.data.files.update(
                                {
                                    "pyproject.toml": await node.data.workspace.read_file(
                                        "pyproject.toml"
                                    )
                                }
                            )
                            result.append(ToolUseResult.from_tool_use(block, "success"))
                    case "complete":
                        if not self.has_modifications(node):
                            raise ValueError(
                                "Can not complete without writing any changes."
                            )
                        check_err = await self.run_checks(node, user_prompt)
                        if check_err:
                            logger.info(f"Failed to complete: {check_err}")
                        result.append(
                            ToolUseResult.from_tool_use(block, check_err or "success")
                        )
                        node.data.should_branch = True
                        is_completed = check_err is None

                    case unknown:
                        raise ValueError(f"Unknown tool: {unknown}")
            except FileNotFoundError as e:
                logger.info(f"File not found: {e}")
                result.append(ToolUseResult.from_tool_use(block, str(e), is_error=True))
            except PermissionError as e:
                logger.info(f"Permission error: {e}")
                result.append(ToolUseResult.from_tool_use(block, str(e), is_error=True))
            except ValueError as e:
                logger.info(f"Value error: {e}")
                result.append(ToolUseResult.from_tool_use(block, str(e), is_error=True))
            except Exception as e:
                logger.error(f"Unknown error: {e}")
                result.append(ToolUseResult.from_tool_use(block, str(e), is_error=True))
        return result, is_completed

    async def eval_node(self, node: Node[BaseData], user_prompt: str) -> bool:
        tool_calls, is_completed = await self.run_tools(node, user_prompt)
        if tool_calls:
            node.data.messages.append(Message(role="user", content=tool_calls))
        else:
            content = [TextRaw(text="Continue or mark completed.")]
            node.data.messages.append(Message(role="user", content=content))
        return is_completed

    def has_modifications(self, node: Node[BaseData]) -> bool:
        cur_node = node
        while cur_node is not None:
            if cur_node.data.files:
                return True
            cur_node = cur_node.parent
        return False

    async def run_type_checks(self, node: Node[BaseData]) -> str | None:
        type_check_result = await node.data.workspace.exec(
            ["uv", "run", "pyright", "."]
        )
        if type_check_result.exit_code != 0:
            return f"{type_check_result.stdout}\n{type_check_result.stderr}"
        return None

    async def run_lint_checks(self, node: Node[BaseData]) -> str | None:
        lint_result = await node.data.workspace.exec(
            ["uv", "run", "ruff", "check", ".", "--fix"]
        )
        if lint_result.exit_code != 0:
            return f"{lint_result.stdout}\n{lint_result.stderr}"
        return None

    async def run_tests(self, node: Node[BaseData]) -> str | None:
        pytest_result = await node.data.workspace.exec_with_pg(["uv", "run", "pytest"])
        if pytest_result.exit_code != 0:
            return f"{pytest_result.stdout}\n{pytest_result.stderr}"
        return None

    async def run_sqlmodel_checks(self, node: Node[BaseData]) -> str | None:
        try:
            await node.data.workspace.read_file("app/database.py")
        except FileNotFoundError:
            return "Database configuration missing: app/database.py file not found"
        smoke_test = await node.data.workspace.exec_with_pg(
            ["uv", "run", "pytest", "-m", "sqlmodel", "-v"]
        )
        if smoke_test.exit_code != 0:
            return (
                f"SQLModel validation failed:\n{smoke_test.stdout}\n{smoke_test.stderr}"
            )
        return None

    async def run_checks(self, node: Node[BaseData], user_prompt: str) -> str | None:
        all_errors = ""
        results = {}

        async with anyio.create_task_group() as tg:

            async def run_and_store(key, coro):
                """Helper to run a coroutine and store its result in the results dict."""
                try:
                    results[key] = await coro
                except Exception as e:
                    # Catch unexpected exceptions during check execution
                    logger.error(f"Error running check {key}: {e}")
                    results[key] = f"Internal error running check {key}: {e}"

            tg.start_soon(run_and_store, "lint", self.run_lint_checks(node))
            tg.start_soon(run_and_store, "type_check", self.run_type_checks(node))
            tg.start_soon(run_and_store, "tests", self.run_tests(node))
            tg.start_soon(run_and_store, "sqlmodel", self.run_sqlmodel_checks(node))

        if lint_result := results.get("lint"):
            logger.info(f"Lint checks failed: {lint_result}")
            all_errors += f"Lint errors:\n{lint_result}\n"
        if type_check_result := results.get("type_check"):
            logger.info(f"Type checks failed: {type_check_result}")
            all_errors += f"Type errors:\n{type_check_result}\n"
        if test_result := results.get("tests"):
            logger.info(f"Tests failed: {test_result}")
            all_errors += f"Test errors:\n{test_result}\n"
        if sqlmodel_result := results.get("sqlmodel"):
            logger.info(f"SQLModel checks failed: {sqlmodel_result}")
            all_errors += f"SQLModel errors:\n{sqlmodel_result}\n"

        if all_errors:
            return all_errors.strip()
        return None

    @property
    def files_allowed(self) -> list[str]:
        return ["app/", "tests/"]

    @property
    def files_protected(self) -> list[str]:
        return [
            "pyproject.toml",
            "main.py",
            "tests/conftest.py",
            "tests/test_sqlmodel_smoke.py",
        ]

    async def get_repo_files(
        self, workspace: Workspace, files: dict[str, str]
    ) -> list[str]:
        repo_files = set(files.keys())
        repo_files.update(
            f"tests/{file_path}" for file_path in await workspace.ls("tests")
        )
        repo_files.update(f"app/{file_path}" for file_path in await workspace.ls("app"))
        # Include root-level files
        root_files = await workspace.ls(".")
        for file_path in root_files:
            if file_path in [
                "docker-compose.yml",
                "Dockerfile",
                "pyproject.toml",
                "main.py",
                "pytest.ini",
            ]:
                repo_files.add(file_path)
        return sorted(list(repo_files))

    async def dump(self) -> object:
        if self.root is None:
            return []
        return await self.dump_node(self.root)

    async def load(self, data: object):
        if not data:
            return
        if not isinstance(data, list):
            raise ValueError(f"Expected list got {type(data)}")
        self.root = await self.load_node(data)
