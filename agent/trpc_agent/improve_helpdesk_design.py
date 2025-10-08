#!/usr/bin/env python3
"""
Improve the design of running HelpDesk app using PowerAppDesignActor.

This script:
1. Reads the current application files from helpdesk_app
2. Uses PowerAppDesignActor to compare with Power App screenshots
3. Iteratively improves the design to match the reference
4. Saves the improved version
"""
import os
import anyio
import logging
import dagger
from llm.utils import get_vision_llm_client, get_best_coding_llm_client
from core.workspace import Workspace
from trpc_agent.actors import PowerAppDesignActor

# Set up logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

# Suppress verbose logs
for package in ["urllib3", "httpx", "google_genai.models"]:
    logging.getLogger(package).setLevel(logging.WARNING)


async def load_app_files(app_dir: str) -> dict[str, str]:
    """Load application source files."""
    files = {}

    # Files to include
    file_patterns = [
        "client/src/App.tsx",
        "client/src/App.css",
        "client/src/components/Dashboard.tsx",
        "client/src/components/MyTickets.tsx",
        "client/src/components/NewTicket.tsx",
        "client/src/components/TicketCard.tsx",
        "client/src/components/TicketList.tsx",
        "server/src/db/schema.ts",
        "server/src/schema.ts",
        "server/src/index.ts",
        "server/src/handlers/create_ticket.ts",
        "server/src/handlers/create_user.ts",
        "server/src/handlers/get_dashboard_stats.ts",
        "server/src/handlers/get_tickets.ts",
        "server/src/handlers/get_user_tickets.ts",
        "server/src/handlers/get_users.ts",
        "server/src/handlers/update_ticket.ts",
        "server/src/tests/create_ticket.test.ts",
        "server/src/tests/create_user.test.ts",
        "server/src/tests/get_dashboard_stats.test.ts",
        "server/src/tests/get_tickets.test.ts",
        "server/src/tests/get_user_tickets.test.ts",
        "server/src/tests/get_users.test.ts",
        "server/src/tests/update_ticket.test.ts",
    ]

    for file_path in file_patterns:
        full_path = os.path.join(app_dir, file_path)
        if os.path.exists(full_path):
            with open(full_path, 'r') as f:
                files[file_path] = f.read()
            logger.info(f"Loaded: {file_path}")
        else:
            logger.warning(f"File not found: {file_path}")

    logger.info(f"Loaded {len(files)} files")
    return files


async def main():
    """Improve HelpDesk application design."""

    app_dir = "./output/helpdesk_app"
    screenshots_path = "/Users/evgenii.kniazev/projects/agent/agent/trpc_agent/screenshots/helpdesk"
    output_dir = "./output/helpdesk_design_improved"

    logger.info("=" * 80)
    logger.info("HELPDESK DESIGN IMPROVEMENT")
    logger.info("=" * 80)

    # Load current application files
    logger.info("\n[STEP 1] Loading current application files...")
    logger.info("-" * 80)

    current_files = await load_app_files(app_dir)

    if not current_files:
        logger.error("No application files found!")
        return

    # Verify screenshots exist
    if not os.path.exists(screenshots_path):
        logger.error(f"Screenshots path not found: {screenshots_path}")
        return

    logger.info(f"Reference screenshots: {screenshots_path}")

    # Disable OpenTelemetry to avoid configuration issues
    os.environ["OTEL_SDK_DISABLED"] = "true"

    async with dagger.Connection(
        dagger.Config(log_output=open(os.devnull, "w"))
    ) as client:

        logger.info("\n[STEP 2] Initializing PowerAppDesignActor...")
        logger.info("-" * 80)

        # Initialize workspace for design improvements
        workspace = await Workspace.create(
            client=client,
            base_image="oven/bun:1.2.5-alpine",
            context=client.host().directory("./trpc_agent/template"),
            setup_cmd=[["bun", "install"]],
        )

        # Create PowerAppDesignActor
        llm = get_best_coding_llm_client()
        vlm = get_vision_llm_client()

        design_actor = PowerAppDesignActor(
            llm=llm,
            vlm=vlm,
            workspace=workspace,
            beam_width=1,
            max_depth=10,
            max_design_iterations=5,
            target_match_score=8,  # Aim for 8/10 match
        )

        # User prompt describing the HelpDesk app
        user_prompt = """
HelpDesk ticket management system with:
- Ticket creation form (Title, Commentary, Category selector, Save/Reset buttons)
- Sidebar navigation with icons (Home, New Ticket, List, Admin)
- Blue color scheme matching Power Apps design
- Pie chart showing ticket status breakdown
- Card showing "My Tickets" count
- Professional, clean interface
""".strip()

        logger.info("\n[STEP 3] Running design improvement iterations...")
        logger.info("-" * 80)

        # Apply design improvements
        try:
            improved_node = await design_actor.execute(
                files=current_files,
                user_prompt=user_prompt,
                reference_screenshots_path=screenshots_path,
                mode="full",
            )

            logger.info("✅ Design improvement completed!")

            # Collect improved files
            improved_files = {}
            for node in improved_node.get_trajectory():
                improved_files.update(node.data.files)

            # Save improved files
            os.makedirs(output_dir, exist_ok=True)

            for file_path, content in improved_files.items():
                if content is not None:
                    full_path = os.path.join(output_dir, file_path)
                    os.makedirs(os.path.dirname(full_path), exist_ok=True)
                    with open(full_path, 'w') as f:
                        f.write(content)

            logger.info(f"Saved improved files to: {output_dir}")

            # Show what changed
            logger.info("\n" + "=" * 80)
            logger.info("CHANGED FILES:")
            logger.info("=" * 80)

            for file_path in sorted(improved_files.keys()):
                if file_path in current_files and improved_files[file_path] != current_files[file_path]:
                    logger.info(f"  ✎ {file_path}")

        except Exception as e:
            logger.error(f"Design improvement failed: {e}")
            logger.exception(e)

    logger.info("\n" + "=" * 80)
    logger.info("DESIGN IMPROVEMENT COMPLETE")
    logger.info("=" * 80)
    logger.info(f"Improved files saved to: {os.path.abspath(output_dir)}")


if __name__ == "__main__":
    anyio.run(main)
