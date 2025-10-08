#!/usr/bin/env python3
"""
Generate a HelpDesk application similar to Power Apps sample and improve design using screenshots.

This script:
1. Generates a HelpDesk ticket management application using TrpcActor
2. Improves the design using PowerAppDesignActor to match Power Apps screenshots
"""
import os
import anyio
import logging
import dagger
from llm.utils import get_vision_llm_client, get_best_coding_llm_client
from core.workspace import Workspace
from core.dagger_utils import write_files_bulk
from trpc_agent.actors import TrpcActor, PowerAppDesignActor
from trpc_agent.application import FSMApplication

# Set up logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

# Suppress verbose logs
for package in ["urllib3", "httpx", "google_genai.models"]:
    logging.getLogger(package).setLevel(logging.WARNING)


async def main():
    """Generate and improve HelpDesk application."""

    # HelpDesk application prompt based on Power Apps sample
    user_prompt = """
Create a modern HelpDesk ticket management system with the following features:

1. **Ticket Management**:
   - Create new tickets with: Title, Commentary/Description, Category (IT, HR, Facilities, Other)
   - View list of all tickets
   - View tickets assigned to current user
   - Track ticket status: Open, In Process, Closed, Cancelled

2. **Dashboard**:
   - Display total number of tickets
   - Show count of tickets assigned to current user
   - Pie chart showing tickets by status (Open, In Process, Closed, Cancelled)
   - Statistics by category

3. **User Interface**:
   - Clean, professional design
   - Navigation sidebar with icons for: Home, New Ticket, Ticket List, Admin Dashboard
   - User profile section showing current user name
   - Responsive layout

4. **Data Model**:
   - Tickets table with: id, title, description, category, status, created_by, assigned_to, created_at
   - Categories: IT, HR, Facilities, Other
   - Status values: Open, In Process, Closed, Cancelled
   - Track creation timestamps

Build a fully functional application with proper database schema, API handlers, and React frontend.
""".strip()

    logger.info("=" * 80)
    logger.info("HELPDESK APPLICATION GENERATION")
    logger.info("=" * 80)

    # Disable OpenTelemetry to avoid configuration issues
    os.environ["OTEL_SDK_DISABLED"] = "true"

    async with dagger.Connection(
        dagger.Config(log_output=open(os.devnull, "w"))
    ) as client:

        # Step 1: Generate the application using FSMApplication
        logger.info("\n[STEP 1] Generating HelpDesk application...")
        logger.info("-" * 80)

        fsm_app = await FSMApplication.start_fsm(
            client,
            user_prompt,
            settings={
                "beam_width": 1,  # Use narrow beam for faster generation
                "max_depth": 50,
            }
        )

        # Run through state machine until complete
        while not fsm_app.is_completed:
            current_state = fsm_app.current_state
            logger.info(f"Current state: {current_state}")

            if fsm_app.maybe_error():
                logger.error(f"Error occurred: {fsm_app.maybe_error()}")
                break

            await fsm_app.confirm_state()

        # Check if generation succeeded
        if fsm_app.maybe_error():
            logger.error(f"Application generation failed: {fsm_app.maybe_error()}")
            return

        logger.info("✅ Application generated successfully!")
        logger.info(f"Generated {len(fsm_app.fsm.context.files)} files")

        # Save generated files to output directory
        output_dir = "./output/helpdesk_generated"
        os.makedirs(output_dir, exist_ok=True)

        for file_path, content in fsm_app.fsm.context.files.items():
            if content is not None:
                full_path = os.path.join(output_dir, file_path)
                os.makedirs(os.path.dirname(full_path), exist_ok=True)
                with open(full_path, 'w') as f:
                    f.write(content)

        logger.info(f"Saved generated files to: {output_dir}")

        # Step 2: Improve design using PowerAppDesignActor
        logger.info("\n[STEP 2] Improving design to match Power Apps screenshots...")
        logger.info("-" * 80)

        screenshots_path = "/Users/evgenii.kniazev/projects/agent/agent/trpc_agent/screenshots/helpdesk"

        # Verify screenshots exist
        if not os.path.exists(screenshots_path):
            logger.warning(f"Screenshots path not found: {screenshots_path}")
            logger.warning("Skipping design improvement step")
            return

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

        # Apply design improvements
        output_dir_improved = "./output/helpdesk_improved"
        try:
            improved_node = await design_actor.execute(
                files=fsm_app.fsm.context.files,
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
            os.makedirs(output_dir_improved, exist_ok=True)

            for file_path, content in improved_files.items():
                if content is not None:
                    full_path = os.path.join(output_dir_improved, file_path)
                    os.makedirs(os.path.dirname(full_path), exist_ok=True)
                    with open(full_path, 'w') as f:
                        f.write(content)

            logger.info(f"Saved improved files to: {output_dir_improved}")

        except Exception as e:
            logger.error(f"Design improvement failed: {e}")
            logger.exception(e)

        logger.info("\n" + "=" * 80)
        logger.info("HELPDESK APPLICATION GENERATION COMPLETE")
        logger.info("=" * 80)
        logger.info(f"Generated app: {os.path.abspath(output_dir)}")
        if os.path.exists(output_dir_improved):
            logger.info(f"Improved app:  {os.path.abspath(output_dir_improved)}")


if __name__ == "__main__":
    anyio.run(main)
