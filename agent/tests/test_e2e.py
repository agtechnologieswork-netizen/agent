import os
import pytest
import tempfile
import anyio
import contextlib
import re

from fire import Fire
from api.agent_server.agent_client import AgentApiClient, MessageKind
from api.agent_server.agent_api_client import (
    apply_patch,
    latest_unified_diff,
    DEFAULT_APP_REQUEST,
    DEFAULT_EDIT_REQUEST,
    spawn_local_server,
    get_all_files_from_project_dir,
)
from api.docker_utils import (
    setup_docker_env,
    start_docker_compose,
    wait_for_healthy_containers,
    stop_docker_compose,
    get_container_logs,
)
from log import get_logger
from tests.test_utils import requires_llm_provider, requires_llm_provider_reason

logger = get_logger(__name__)

pytestmark = pytest.mark.anyio


@contextlib.contextmanager
def empty_context():
    yield


@pytest.fixture
def anyio_backend():
    return "asyncio"


def latest_app_name_and_commit_message(events):
    """Extract the most recent app_name and commit_message from events"""
    app_name = None
    commit_message = None

    for evt in reversed(events):
        try:
            if evt.message:
                # Update app_name if found and not yet set
                if app_name is None and evt.message.app_name is not None:
                    app_name = evt.message.app_name

                # Update commit_message if found and not yet set
                if commit_message is None and evt.message.commit_message is not None:
                    commit_message = evt.message.commit_message

                # If both are set, we can break
                if app_name is not None and commit_message is not None:
                    break
        except AttributeError:
            continue

    return app_name, commit_message


def extract_powerapp_source_path(prompt: str) -> str | None:
    """
    Extract PowerApp source path from prompt.
    Looks for patterns like "from /path/to/powerapp" or "powerapp from /path"
    Returns the first valid directory path found.
    """
    # Match patterns like "from /path" or "powerapp: /path"
    patterns = [
        r'powerapp\s+from\s+([/~][^\s]+)',  # powerapp from /path (most specific)
        r'from\s+([/~][^\s]+)\s+source',  # from /path source
        r'migrate.*?from\s+([/~][^\s]+)',  # migrate ... from /path
    ]

    for pattern in patterns:
        match = re.search(pattern, prompt, re.IGNORECASE)
        if match:
            path = match.group(1)
            # Expand ~ to home directory
            if path.startswith('~'):
                path = os.path.expanduser(path)
            # Check if path exists
            if os.path.isdir(path):
                logger.info(f"Detected PowerApp source path: {path}")
                return path

    return None


def extract_screenshot_path(prompt: str) -> str | None:
    """
    Extract explicit screenshot path from prompt.
    Looks for patterns like "improve visually from /path" or "screenshots from /path"
    """
    patterns = [
        r'(?:improve|enhance)\s+visually\s+from\s+([/~][^\s]+)',  # improve visually from /path
        r'screenshots?\s+(?:from|in|at)\s+([/~][^\s]+)',  # screenshot(s) from/in/at /path
        r'visual(?:s|ly)?\s+from\s+([/~][^\s]+)',  # visual(s)/visually from /path
    ]

    for pattern in patterns:
        match = re.search(pattern, prompt, re.IGNORECASE)
        if match:
            path = match.group(1)
            # Expand ~ to home directory
            if path.startswith('~'):
                path = os.path.expanduser(path)
            # Check if path exists and has images
            if os.path.isdir(path):
                has_images = any(
                    f.lower().endswith(('.png', '.jpg', '.jpeg'))
                    for f in os.listdir(path)
                )
                if has_images:
                    logger.info(f"Detected screenshot path from prompt: {path}")
                    return path

    return None


def find_powerapp_screenshots(source_path: str) -> str | None:
    """
    Find screenshot directory in PowerApp source.
    Looks for common screenshot folder names.
    """
    screenshot_dirs = ['screenshots', 'screens', 'images', 'assets']

    for dir_name in screenshot_dirs:
        screenshot_path = os.path.join(source_path, dir_name)
        if os.path.isdir(screenshot_path):
            # Check if there are any image files
            has_images = any(
                f.lower().endswith(('.png', '.jpg', '.jpeg'))
                for f in os.listdir(screenshot_path)
            )
            if has_images:
                logger.info(f"Found screenshots in: {screenshot_path}")
                return screenshot_path

    logger.warning(f"No screenshot directory found in {source_path}")
    return None


async def run_e2e(
    prompt: str,
    standalone: bool,
    with_edit=True,
    template_id=None,
    use_databricks=False,
    output_dir=None,
):
    context = empty_context() if standalone else spawn_local_server()
    settings = {}
    if use_databricks:
        settings = {
            "databricks_host": os.getenv("DATABRICKS_HOST"),
            "databricks_token": os.getenv("DATABRICKS_TOKEN"),
        }
        if not settings["databricks_host"] or not settings["databricks_token"]:
            raise ValueError(
                "Databricks host and token must be set in environment variables to use Databricks"
            )

    with context:
        async with AgentApiClient() as client:
            events, request = await client.send_message(
                prompt, template_id=template_id, settings=settings
            )
            assert events, "No response received from agent"
            max_refinements = 5
            refinement_count = 0

            while (
                events[-1].message.kind == MessageKind.REFINEMENT_REQUEST
                and refinement_count < max_refinements
            ):
                events, request = await client.continue_conversation(
                    previous_events=events,
                    previous_request=request,
                    message="just do it! no more questions, please",
                    template_id=template_id,
                    settings=settings,
                )
                refinement_count += 1
                logger.info(f"Refinement attempt {refinement_count}/{max_refinements}")

            if refinement_count >= max_refinements:
                logger.error("Maximum refinement attempts exceeded")
                raise RuntimeError(
                    "Agent stuck in refinement loop - exceeded maximum attempts"
                )

            diff = latest_unified_diff(events)
            assert diff, "No diff was generated in the agent response"

            # Check that app_name and commit_message are present in the response
            app_name, commit_message = latest_app_name_and_commit_message(events)
            assert app_name is not None, (
                "No app_name was generated in the agent response"
            )
            assert commit_message is not None, (
                "No commit_message was generated in the agent response"
            )
            logger.info(f"Generated app_name: {app_name}")
            logger.info(f"Generated commit_message: {commit_message}")

            # Use output_dir if provided, otherwise create temporary directory
            if output_dir:
                os.makedirs(output_dir, exist_ok=True)
                temp_dir = output_dir
                temp_dir_context = contextlib.nullcontext()
            else:
                temp_dir_context = tempfile.TemporaryDirectory()

            with temp_dir_context as managed_dir:
                temp_dir = temp_dir if output_dir else managed_dir
                # Determine template path based on template_id
                template_paths = {
                    "nicegui_agent": "nicegui_agent/template",
                    "trpc_agent": "trpc_agent/template",
                    "laravel_agent": "laravel_agent/template",
                    None: "trpc_agent/template",  # default
                }

                # Apply the first diff
                success, message = apply_patch(
                    diff, temp_dir, template_paths[template_id]
                )
                assert success, f"Failed to apply first patch: {message}"

                if with_edit:
                    # Read all files from the patched directory to provide as context
                    files_for_snapshot = get_all_files_from_project_dir(temp_dir)
                    all_files = [f.model_dump() for f in files_for_snapshot]

                    new_events, new_request = await client.continue_conversation(
                        previous_events=events,
                        previous_request=request,
                        message=DEFAULT_EDIT_REQUEST,
                        all_files=all_files,
                        template_id=template_id,
                        settings=settings,
                    )
                    
                    # Handle potential refinement requests after edit
                    refinement_count = 0
                    while (
                        new_events
                        and new_events[-1].message.kind == MessageKind.REFINEMENT_REQUEST
                        and refinement_count < max_refinements
                    ):
                        new_events, new_request = await client.continue_conversation(
                            previous_events=new_events,
                            previous_request=new_request,
                            message="just do it! no more questions, please",
                            template_id=template_id,
                            settings=settings,
                        )
                        refinement_count += 1
                        logger.info(f"Edit refinement attempt {refinement_count}/{max_refinements}")
                    
                    updated_diff = latest_unified_diff(new_events)
                    assert updated_diff, (
                        "No diff was generated in the agent response after edit"
                    )
                    assert updated_diff != diff, "Edit did not produce a new diff"

                    # Apply the second diff (incremental on top of first)
                    success, message = apply_patch(
                        updated_diff, temp_dir, template_paths[template_id]
                    )
                    assert success, f"Failed to apply second patch: {message}"

                # Check if this is a PowerApp migration and apply design improvements
                powerapp_source = extract_powerapp_source_path(prompt)
                if powerapp_source and (template_id == "trpc_agent" or template_id is None):
                    # Try explicit screenshot path from prompt first
                    screenshot_path = extract_screenshot_path(prompt)
                    # Fall back to searching in PowerApp source
                    if not screenshot_path:
                        screenshot_path = find_powerapp_screenshots(powerapp_source)

                    if screenshot_path:
                        logger.info("🎨 Applying PowerApp design improvements...")
                        try:
                            import dagger
                            from trpc_agent.actors import PowerAppDesignActor
                            from llm.utils import get_vision_llm_client, get_best_coding_llm_client
                            from core.workspace import Workspace

                            # Load generated files
                            files_dict = {}
                            for root, _, filenames in os.walk(temp_dir):
                                for filename in filenames:
                                    if filename.endswith(('.ts', '.tsx', '.css', '.json')):
                                        file_path = os.path.join(root, filename)
                                        rel_path = os.path.relpath(file_path, temp_dir)
                                        try:
                                            with open(file_path, 'r') as f:
                                                files_dict[rel_path] = f.read()
                                        except Exception as e:
                                            logger.warning(f"Failed to read {rel_path}: {e}")

                            if files_dict:
                                # Create dagger client and workspace
                                async with dagger.Connection(
                                    dagger.Config(log_output=open(os.devnull, "w"))
                                ) as dagger_client:
                                    workspace = await Workspace.create(
                                        client=dagger_client,
                                        base_image="oven/bun:1.2.5-alpine",
                                        context=dagger_client.host().directory(temp_dir),
                                        setup_cmd=[["bun", "install"]],
                                    )

                                    # Initialize LLM clients
                                    llm = get_best_coding_llm_client()
                                    vlm = get_vision_llm_client()

                                    # Create actor
                                    actor = PowerAppDesignActor(
                                        llm=llm,
                                        vlm=vlm,
                                        workspace=workspace,
                                        max_design_iterations=5,
                                        target_match_score=8,
                                    )

                                    # Run design improvement
                                    result_node = await actor.execute(
                                        files=files_dict,
                                        user_prompt=prompt,
                                        reference_screenshots_path=screenshot_path,
                                        mode="client",
                                    )

                                    # Write improved files back to temp_dir
                                    for file_path, content in result_node.data.files.items():
                                        if content is not None:  # None means file was deleted
                                            full_path = os.path.join(temp_dir, file_path)
                                            os.makedirs(os.path.dirname(full_path), exist_ok=True)
                                            with open(full_path, 'w') as f:
                                                f.write(content)
                                            logger.info(f"✅ Updated: {file_path}")

                                    logger.info("🎨 Design improvements applied successfully!")
                        except Exception as e:
                            logger.error(f"Failed to apply design improvements: {e}", exc_info=True)
                            # Continue anyway - design improvement is optional
                    else:
                        logger.info("No PowerApp screenshots found, skipping design improvement")

                original_dir = os.getcwd()
                container_names = setup_docker_env()

                try:
                    os.chdir(temp_dir)

                    success, error_message = start_docker_compose(
                        temp_dir, container_names["project_name"]
                    )
                    if not success:
                        # Get logs if possible for debugging
                        try:
                            logs = get_container_logs(
                                [
                                    container_names["db_container_name"],
                                    container_names["app_container_name"],
                                ]
                            )
                            for container, log in logs.items():
                                logger.error(f"Container {container} logs: {log}")
                        except Exception:
                            logger.error("Failed to get container logs")

                        logger.error(
                            f"Error starting Docker containers: {error_message}"
                        )
                        raise RuntimeError(
                            f"Failed to start Docker containers: {error_message}"
                        )

                    container_healthy = await wait_for_healthy_containers(
                        [
                            container_names["db_container_name"],
                            container_names["app_container_name"],
                        ],
                        ["db", "app"],
                        timeout=60,
                        interval=1,
                    )

                    if not container_healthy:
                        raise RuntimeError(
                            "Containers did not become healthy within the timeout period"
                        )

                    if standalone:
                        if output_dir:
                            input(
                                f"App is running on http://localhost:80/, app saved to {temp_dir}; Press Enter to tear down containers (app directory will be kept)..."
                            )
                        else:
                            input(
                                f"App is running on http://localhost:80/, app dir is {temp_dir}; Press Enter to continue and tear down..."
                            )
                        print("🧹Tearing down containers... ")

                finally:
                    # Restore original directory
                    os.chdir(original_dir)

                    # Clean up Docker containers
                    stop_docker_compose(temp_dir, container_names["project_name"])


@pytest.mark.parametrize(
    "template_id",
    [
        pytest.param("nicegui_agent", marks=pytest.mark.nicegui),
    ],
)
async def test_e2e_generation_nicegui(template_id):
    await run_e2e(standalone=False, prompt=DEFAULT_APP_REQUEST, template_id=template_id)


@pytest.mark.skipif(requires_llm_provider(), reason=requires_llm_provider_reason)
@pytest.mark.parametrize(
    "template_id", [pytest.param("trpc_agent", marks=pytest.mark.trpc)]
)
async def test_e2e_generation_trpc(template_id):
    await run_e2e(standalone=False, prompt=DEFAULT_APP_REQUEST, template_id=template_id)

@pytest.mark.skipif(requires_llm_provider(), reason=requires_llm_provider_reason)
@pytest.mark.parametrize(
    "template_id", [pytest.param("laravel_agent", marks=pytest.mark.laravel)]
)
async def test_e2e_generation_laravel(template_id):
    await run_e2e(standalone=False, prompt=DEFAULT_APP_REQUEST, template_id=template_id)


def create_app(prompt, output_dir=None):
    import coloredlogs

    coloredlogs.install(level="INFO")
    anyio.run(run_e2e, prompt, True, output_dir=output_dir)


if __name__ == "__main__":
    Fire(create_app)
