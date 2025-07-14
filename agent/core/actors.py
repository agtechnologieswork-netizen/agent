from typing import Protocol
import dataclasses
import anyio
from anyio.streams.memory import MemoryObjectSendStream
from core import statemachine
from core.base_node import Node
from llm.common import AsyncLLM, Message
from llm.utils import loop_completion
from core.workspace import Workspace
import hashlib
from abc import ABC, abstractmethod
from llm.common import Tool, ToolUse, ToolUseResult, TextRaw
from log import get_logger

logger = get_logger(__name__)


@dataclasses.dataclass
class BaseData:
    workspace: Workspace
    messages: list[Message]
    files: dict[str, str | None] = dataclasses.field(default_factory=dict)
    should_branch: bool = False

    def head(self) -> Message:
        if (num_messages := len(self.messages)) != 1:
            raise ValueError(f"Expected 1 got {num_messages} messages: {self.messages}")
        if self.messages[0].role != "assistant":
            raise ValueError(f"Expected assistant role in message: {self.messages}")
        return self.messages[0]

    @property
    def file_cache_key(self) -> str:
        s = ""
        for file, content in sorted(self.files.items(), key=lambda x: x[0]):
            s += f"{file}:{content}"
        return hashlib.md5(s.encode()).hexdigest()


class BaseActor(statemachine.Actor):
    workspace: Workspace

    async def dump_data(self, data: BaseData) -> object:
        return {
            "messages": [msg.to_dict() for msg in data.messages],
            "files": data.files,
            "should_branch": data.should_branch,
        }

    async def load_data(self, data: dict, workspace: Workspace) -> BaseData:
        for file, content in data["files"].items():
            if content is not None:
                workspace.write_file(file, content)
            else:
                workspace.rm(file)
        messages = [Message.from_dict(msg) for msg in data["messages"]]
        return BaseData(
            workspace, messages, data["files"], data.get("should_branch", False)
        )

    async def dump_node(self, node: Node[BaseData]) -> list[dict]:
        stack, result = [node], []
        while stack:
            node = stack.pop()
            result.append(
                {
                    "id": node._id,
                    "parent": node.parent._id if node.parent else None,
                    "data": await self.dump_data(node.data),
                }
            )
            stack.extend(node.children)
        return result

    async def load_node(self, data: list[dict]) -> Node[BaseData]:
        root = None
        id_to_node: dict[str, Node[BaseData]] = {}
        for item in data:
            parent = id_to_node[item["parent"]] if item["parent"] else None
            workspace = parent.data.workspace if parent else self.workspace
            node_data = await self.load_data(item["data"], workspace.clone())
            node = Node(node_data, parent, item["id"])
            if parent:
                parent.children.append(node)
            else:
                root = node
            id_to_node[item["id"]] = node
        if root is None:
            raise ValueError("Root node not found")
        return root

    async def dump(self) -> object: ...

    async def load(self, data: object): ...


class LLMActor(Protocol):
    llm: AsyncLLM

    async def run_llm(
        self, nodes: list[Node[BaseData]], system_prompt: str | None = None, **kwargs
    ) -> list[Node[BaseData]]:
        async def node_fn(
            node: Node[BaseData], tx: MemoryObjectSendStream[Node[BaseData]]
        ):
            history = [m for n in node.get_trajectory() for m in n.data.messages]
            new_node = Node[BaseData](
                data=BaseData(
                    workspace=node.data.workspace.clone(),
                    messages=[
                        await loop_completion(
                            self.llm, history, system_prompt=system_prompt, **kwargs
                        )
                    ],
                ),
                parent=node,
            )
            async with tx:
                await tx.send(new_node)

        result = []
        tx, rx = anyio.create_memory_object_stream[Node[BaseData]]()
        async with anyio.create_task_group() as tg:
            for node in nodes:
                tg.start_soon(node_fn, node, tx.clone())
            tx.close()
            async with rx:
                async for new_node in rx:
                    new_node.parent.children.append(new_node)  # pyright: ignore[reportOptionalMemberAccess]
                    result.append(new_node)
        return result


class FileOperationsActor(BaseActor, LLMActor, ABC):
    """Base class for actors that perform file operations with common tools."""

    def __init__(
        self,
        llm: AsyncLLM,
        workspace: Workspace,
        beam_width: int = 3,
        max_depth: int = 30,
    ):
        self.llm = llm
        self.workspace = workspace
        self.beam_width = beam_width
        self.max_depth = max_depth
        self.root = None
        logger.info(
            f"Initialized {self.__class__.__name__} with beam_width={beam_width}, max_depth={max_depth}"
        )

    @property
    def base_tools(self) -> list[Tool]:
        """Common file operation tools."""
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
                "name": "complete",
                "description": "Mark the task as complete. This will run tests and type checks to ensure the changes are correct.",
                "input_schema": {
                    "type": "object",
                    "properties": {},
                },
            },
        ]

    @property
    def additional_tools(self) -> list[Tool]:
        """Additional tools specific to the subclass. Override in subclasses."""
        return []

    @property
    def tools(self) -> list[Tool]:
        """All tools available to this actor."""
        return self.base_tools + self.additional_tools

    def _short_dict_repr(self, d: dict) -> str:
        """Helper function to create short dictionary representations for logging."""
        return ", ".join(
            f"{k}: {v if len(v) < 100 else v[:50] + '...'}"
            for k, v in d.items()
            if isinstance(v, str)
        )

    async def handle_custom_tool(
        self, tool_use: ToolUse, node: Node[BaseData]
    ) -> ToolUseResult:
        """Handle custom tools specific to subclasses. Override in subclasses."""
        raise ValueError(f"Unknown tool: {tool_use.name}")

    async def run_tools(
        self, node: Node[BaseData], user_prompt: str
    ) -> tuple[list[ToolUseResult], bool]:
        """Execute tools for a given node."""
        logger.info(f"Running tools for node {node._id}")
        result, is_completed = [], False

        for block in node.data.head().content:
            if not isinstance(block, ToolUse):
                match block:
                    case TextRaw(text=text):
                        logger.info(f"LLM output: {text}")
                    case _:
                        pass
                continue

            try:
                logger.info(
                    f"Running tool {block.name} with input {self._short_dict_repr(block.input) if isinstance(block.input, dict) else str(block.input)}"
                )

                match block.name:
                    case "read_file":
                        tool_content = await node.data.workspace.read_file(
                            block.input["path"]  # pyright: ignore[reportIndexIssue]
                        )
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
                            error_msg = (
                                f"Directory not found for file '{path}': {str(e)}"
                            )
                            logger.info(
                                f"File not found error writing file {path}: {str(e)}"
                            )
                            result.append(
                                ToolUseResult.from_tool_use(
                                    block, error_msg, is_error=True
                                )
                            )
                        except PermissionError as e:
                            error_msg = (
                                f"Permission denied writing file '{path}': {str(e)}"
                            )
                            logger.info(
                                f"Permission error writing file {path}: {str(e)}"
                            )
                            result.append(
                                ToolUseResult.from_tool_use(
                                    block, error_msg, is_error=True
                                )
                            )
                        except ValueError as e:
                            error_msg = str(e)
                            logger.info(f"Value error writing file {path}: {error_msg}")
                            result.append(
                                ToolUseResult.from_tool_use(
                                    block, error_msg, is_error=True
                                )
                            )

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
                                    result.append(
                                        ToolUseResult.from_tool_use(block, "success")
                                    )
                                    logger.debug(f"Applied edit to file: {path}")
                                case num_hits:
                                    raise ValueError(
                                        f"Search text found {num_hits} times in file '{path}' (expected exactly 1). Search:\n{search}"
                                    )
                        except FileNotFoundError as e:
                            error_msg = f"File '{path}' not found for editing: {str(e)}"
                            logger.info(
                                f"File not found error editing file {path}: {str(e)}"
                            )
                            result.append(
                                ToolUseResult.from_tool_use(
                                    block, error_msg, is_error=True
                                )
                            )
                        except PermissionError as e:
                            error_msg = (
                                f"Permission denied editing file '{path}': {str(e)}"
                            )
                            logger.info(
                                f"Permission error editing file {path}: {str(e)}"
                            )
                            result.append(
                                ToolUseResult.from_tool_use(
                                    block, error_msg, is_error=True
                                )
                            )
                        except ValueError as e:
                            error_msg = str(e)
                            logger.info(f"Value error editing file {path}: {error_msg}")
                            result.append(
                                ToolUseResult.from_tool_use(
                                    block, error_msg, is_error=True
                                )
                            )

                    case "delete_file":
                        node.data.workspace.rm(block.input["path"])  # pyright: ignore[reportIndexIssue]
                        node.data.files.update({block.input["path"]: None})  # pyright: ignore[reportIndexIssue]
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

                    case _:
                        # Handle custom tools via subclass
                        if isinstance(block.input, dict):
                            custom_result = await self.handle_custom_tool(
                                block, node
                            )
                            result.append(custom_result)
                        else:
                            raise ValueError(
                                f"Invalid input type for tool {block.name}: {type(block.input)}"
                            )

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
        """Evaluate a node by running its tools."""
        tool_calls, is_completed = await self.run_tools(node, user_prompt)
        if tool_calls:
            node.data.messages.append(Message(role="user", content=tool_calls))
        else:
            content = [TextRaw(text="Continue or mark completed via tool.")]
            node.data.messages.append(Message(role="user", content=content))
        return is_completed

    def has_modifications(self, node: Node[BaseData]) -> bool:
        """Check if the node or any of its ancestors have file modifications."""
        cur_node = node
        while cur_node is not None:
            if cur_node.data.files:
                return True
            cur_node = cur_node.parent
        return False

    @abstractmethod
    async def run_checks(self, node: Node[BaseData], user_prompt: str) -> str | None:
        """Run validation checks. Must be implemented by subclasses."""
        pass

    async def dump(self) -> object:
        """Dump the actor state."""
        if self.root is None:
            return []
        return await self.dump_node(self.root)

    async def load(self, data: object):
        """Load the actor state."""
        if not data:
            return
        if not isinstance(data, list):
            raise ValueError(f"Expected list got {type(data)}")
        self.root = await self.load_node(data)
