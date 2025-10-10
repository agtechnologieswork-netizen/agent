# PowerApp Design Integration

## Overview

The PowerAppDesignActor has been successfully integrated into the main `uv run generate` command flow to automatically improve generated app designs based on PowerApp screenshots.

## How It Works

When you run a PowerApp migration command, the system will:

1. **Detect PowerApp Source**: Automatically extract the PowerApp source path from your prompt
2. **Find Screenshots**: Look for screenshot directories in the source (screenshots/, screens/, images/, assets/)
3. **Generate App**: Use the standard generation flow to create the initial app
4. **Apply Design Improvements**: Run PowerAppDesignActor to iteratively match the design
5. **Launch App**: Start the improved app in Docker

## Usage

### Full Generation with Design Improvement

```bash
# Migrate a PowerApp with automatic design improvement
uv run generate "migrate a powerapp from /path/to/powerapp-source"

# Example with the HelpDesk sample
uv run generate "migrate a powerapp from /tmp/powerapps-samples/samples/HelpDesk-theme source code"

# With persistent output directory
uv run generate \
  --output_dir=./output/my-helpdesk-app \
  "migrate a powerapp from /path/to/powerapp-source"

# With explicit screenshot path
uv run generate \
  --output_dir=./output/my-app \
  "migrate a powerapp from /path/to/source and improve visually from /path/to/screenshots"
```

### Design Improvement Only (Existing App)

If you already have a generated app and want to run only the design improvement:

```bash
# Run design improvements on an existing app
uv run improve_design \
  --app_dir=./output/my-helpdesk-app \
  --screenshot_path=/path/to/screenshots \
  --prompt="Match the PowerApp design" \
  --max_iterations=5 \
  --target_score=8

# Alternative: using Python module directly
python -m tests.test_e2e improve_design \
  --app_dir=./output/my-app \
  --screenshot_path=/path/to/screenshots
```

## Requirements

- PowerApp source directory must contain a `screenshots/` (or similar) folder with PNG/JPEG images
- Screenshots should show the main views of the PowerApp
- Both `ANTHROPIC_API_KEY` and `GEMINI_API_KEY` environment variables must be set

## Design Improvement Process

The PowerAppDesignActor:

1. Captures screenshots of the generated app using Playwright
2. Compares them with PowerApp reference screenshots using Gemini Vision AI
3. Identifies design differences (colors, layout, typography, spacing)
4. Uses Claude to generate CSS/styling changes
5. Applies changes and re-tests
6. Repeats for up to 5 iterations or until match score ≥ 8/10

## Configuration

Default settings in the integration:
- **Max iterations**: 5
- **Target match score**: 8/10
- **Mode**: "client" (only runs client-side screenshots for faster iterations)

## Files Modified

- `tests/test_e2e.py`: Added PowerApp detection and design improvement integration
- `trpc_agent/actors.py`: Added PowerAppDesignActor class and fixed eval_node method
- `trpc_agent/playbooks.py`: Added design comparison and improvement prompts
- `trpc_agent/playwright.py`: Added compare_with_reference() method
- `trpc_agent/screenshots/README.md`: Documentation for screenshot storage

## Key Features

✅ **Automatic Detection**: Prompts mentioning PowerApp sources are automatically detected
✅ **Vision-Based Comparison**: Uses Gemini 2.5 Flash for screenshot analysis
✅ **Iterative Improvement**: Applies changes in multiple rounds for better matching
✅ **Non-Breaking**: Design improvements are optional - generation continues even if they fail
✅ **CSS-Only Changes**: Only modifies styling, preserves all functionality
✅ **Robust Error Handling**: Errors during improvement don't crash the pipeline
✅ **Standalone Mode**: Run design improvements on existing apps without regeneration
✅ **Persistent Output**: Keep generated apps with `--output_dir` parameter

## Troubleshooting

### No screenshots found
- Ensure your PowerApp source has a `screenshots/`, `screens/`, `images/`, or `assets/` directory
- Check that the directory contains .png or .jpg files

### Design improvements not running
- Verify both API keys are set: `ANTHROPIC_API_KEY` and `GEMINI_API_KEY`
- Check that the prompt mentions a PowerApp source path
- Only works with tRPC template (trpc_agent)

### Low match scores
- Higher quality screenshots lead to better results
- Ensure screenshots show the actual rendered UI, not just mockups
- Some design elements may require manual tuning

### Design improvement errors
- The pipeline will continue even if design improvements fail
- Check logs for detailed error messages (look for "❌ Failed to apply design improvements")
- User can interrupt with Ctrl+C without losing generated app (if using `--output_dir`)
- Try running `uv run improve_design` separately on the generated app to retry

### App still launches with errors
- Design improvement is optional - the app will launch even if improvements fail
- This ensures you always get a working app, even if visual matching has issues
- You can manually tweak the design or re-run `improve_design` later

## Example Prompt Patterns

These patterns will trigger automatic design improvement:

- `migrate a powerapp from /path/to/source`
- `create an app based on powerapp: /path/to/source`
- `migrate /path/to/powerapp-source to React`
- `build app from /path/to/powerapp`

## Future Enhancements

- Support for more screenshot formats (WebP, SVG)
- Configurable iteration count and match score threshold
- Component-level design matching
- Design diff visualization
