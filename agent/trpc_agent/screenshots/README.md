# PowerApp Screenshots for Design Improvement

This directory is for storing PowerApp screenshots that will be used for automated design improvement during app generation.

## How to Use

When migrating a PowerApp, the generation tool will automatically:

1. Detect PowerApp source paths in your prompt
2. Look for screenshot directories in the source (e.g., `screenshots/`, `screens/`, `images/`, `assets/`)
3. Use those screenshots to iteratively improve the generated app's design to match the PowerApp

## Directory Structure

Place screenshots in subdirectories named after the app:

```
screenshots/
├── helpdesk/
│   ├── screen1.png
│   ├── screen2.png
│   └── screen3.png
├── expense-tracker/
│   ├── dashboard.png
│   └── form.png
└── README.md
```

## Example Usage

```bash
# Migrate a PowerApp from a source directory with screenshots
uv run generate "migrate a powerapp from /path/to/powerapp-source"

# The tool will look for screenshots in:
# - /path/to/powerapp-source/screenshots/
# - /path/to/powerapp-source/screens/
# - /path/to/powerapp-source/images/
# - /path/to/powerapp-source/assets/
```

## Design Improvement Process

The PowerAppDesignActor will:

1. Generate the initial app based on your prompt
2. Capture screenshots of the generated app
3. Compare them with reference PowerApp screenshots using Vision AI
4. Identify design differences (colors, layout, typography, spacing)
5. Iteratively apply CSS/styling changes to match the reference
6. Repeat until design match score reaches 8/10 or max iterations (5)

## Supported Image Formats

- PNG (.png)
- JPEG (.jpg, .jpeg)

## Notes

- Screenshots should show the main views/screens of your PowerApp
- Higher quality screenshots lead to better design matching
- The tool focuses on visual design (CSS) only - functionality is not affected
