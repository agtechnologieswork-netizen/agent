import re
import logging
from typing import Callable, Awaitable
from core.base_node import Node
from core.workspace import ExecResult
from core.actors import BaseData
from llm.common import TextRaw
from trpc_agent.notification_utils import notify_if_callback

logger = logging.getLogger(__name__)


class ParseFiles:
    def __init__(self):
        self.pattern = re.compile(
            r'<file path="(?P<path>[^"]+)">(?P<content>.*?)</file>',
            re.DOTALL
        )

    def __call__(self, content: str):
        matches = self.pattern.finditer(content)
        return {match.group("path"): match.group("content") for match in matches}


parse_files = ParseFiles()


async def run_write_files(
    node: Node[BaseData],
    event_callback: Callable[[str], Awaitable[None]] | None = None,
) -> TextRaw | None:
    errors = []
    files_written = 0
    all_files_written = []

    for block in node.data.head().content:
        if not (isinstance(block, TextRaw)):
            continue
        parsed_files = parse_files(block.text)
        for file, content in parsed_files.items():
            try:
                node.data.workspace.write_file(file, content)
                node.data.files.update({file: content})
                files_written += 1
                all_files_written.append(file)
                logger.debug(f"Written file: {file}")
            except PermissionError as e:
                error_msg = str(e)
                logger.info(f"Permission error writing file {file}: {error_msg}")
                errors.append(error_msg)

    if files_written > 0:
        logger.debug(f"Written {files_written} files to workspace")
        # Create user-friendly progress message instead of raw diff
        file_summary = []
        for file in all_files_written[:3]:  # Show first 3 files
            if file.endswith('.ts') or file.endswith('.tsx'):
                file_summary.append(f"📝 {file}")
            elif file.endswith('.css'):
                file_summary.append(f"🎨 {file}")
            elif file.endswith('.json'):
                file_summary.append(f"⚙️ {file}")
            else:
                file_summary.append(f"📄 {file}")
        
        more_files = f" (+{len(all_files_written)-3} more)" if len(all_files_written) > 3 else ""
        progress_msg = f"✨ Generated {files_written} files:\n" + "\n".join(file_summary) + more_files
        await notify_if_callback(event_callback, progress_msg, "progress update")

    if errors:
        errors.append(f"Only those files should be written: {node.data.workspace.allowed}")

    return TextRaw("\n".join(errors)) if errors else None


async def run_tsc_compile(node: Node[BaseData], event_callback: Callable[[str], Awaitable[None]] | None = None) -> tuple[ExecResult, TextRaw | None]:
    logger.debug("Running TypeScript compilation")
    
    await notify_if_callback(event_callback, "🔧 Compiling TypeScript...", "compilation start")
    
    result = await node.data.workspace.exec(["bun", "run", "tsc", "--noEmit", "--incremental"], cwd="server")
    
    if result.exit_code == 0:
        logger.info("TypeScript compilation succeeded")
        await notify_if_callback(event_callback, "✅ TypeScript compilation successful", "compilation success")
        return result, None

    logger.debug(f"TypeScript compilation failed with exit code {result.exit_code}")
    await notify_if_callback(event_callback, "❌ TypeScript compilation failed - fixing errors...", "compilation failure")
    return result, TextRaw(f"Error running tsc: {result.stdout}")




class RunTests:
    def __init__(self):
        self.test_output_normalizer = re.compile(r"\[\d+(\.\d+)?(ms|s)\]")

    async def __call__(self, node: Node[BaseData], event_callback: Callable[[str], Awaitable[None]] | None = None) -> tuple[ExecResult, TextRaw | None]:
        await notify_if_callback(event_callback, "🧪 Running tests...", "test start")
        
        result = await node.data.workspace.exec_with_pg(["bun", "test"], cwd="server")
        
        if result.exit_code == 0:
            await notify_if_callback(event_callback, "✅ All tests passed!", "test success")
            return result, None

        logger.info(f"Tests failed with exit code {result.exit_code}")
        await notify_if_callback(event_callback, "❌ Tests failed - fixing issues...", "test failure")
        
        err = self.test_output_normalizer.sub("", result.stderr)
        err = "\n".join([x.rstrip() for x in err.splitlines()])
        return result, TextRaw(f"Error running tests: {err}")

run_tests = RunTests()

class RunFrontendBuild:
    def __init__(self):
        self.build_output_normalizer = re.compile(r"\d+(\.\d+)?(ms|s)")

    async def __call__(self, node: Node[BaseData], event_callback: Callable[[str], Awaitable[None]] | None = None) -> str | None:
        await notify_if_callback(event_callback, "🏗️ Building frontend...", "build start")
        
        result = await node.data.workspace.exec(["bun", "run", "build"], cwd="client")
        if result.exit_code != 0:
            await notify_if_callback(event_callback, "❌ Frontend build failed - fixing issues...", "build failure")
            err = self.build_output_normalizer.sub("", result.stderr)
            return f"Build errors:\n{err}\n"

        await notify_if_callback(event_callback, "🔍 Running frontend linter...", "lint start")

        result = await node.data.workspace.exec(["bun", "run", "lint"], cwd="client")
        if result.exit_code != 0:
            logger.info(f"Linting failed with exit code {result.exit_code}")
            await notify_if_callback(event_callback, "❌ Linting failed - fixing code style...", "lint failure")
            return f"Lint errors:\n{result.stdout}\n"

        await notify_if_callback(event_callback, "✅ Frontend built successfully!", "build success")

        return None

run_frontend_build = RunFrontendBuild()
