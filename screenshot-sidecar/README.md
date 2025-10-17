# Screenshot Sidecar

Reusable Dagger module for capturing screenshots of web applications using Playwright.

## Architecture

The sidecar is completely independent and generic:
- **Apps** only need a Dockerfile that builds and runs the application
- **Screenshot-sidecar** builds the Dockerfile, starts the service, and captures screenshots
- Zero coupling - sidecar has no knowledge of app-specific details or environment variables

## Usage

### Basic usage

```bash
cd screenshot-sidecar
dagger call screenshot-app \
  --app-source=../dataapps/template_trpc \
  export --path=screenshot.png
```

### With convenience script

```bash
cd screenshot-sidecar
./run-screenshot.sh ../dataapps/template_trpc screenshot.png
```

### With environment variables

Pass any environment variables your app needs as comma-separated KEY=VALUE pairs:

```bash
./run-screenshot.sh \
  ../dataapps/template_trpc \
  screenshot.png \
  "PORT=8000,DATABRICKS_HOST=https://workspace.databricks.com,DATABRICKS_TOKEN=secret"
```

### Direct API call with env vars

```bash
dagger call screenshot-app \
  --app-source=../dataapps/template_trpc \
  --env-vars="PORT=8000,DEBUG=true" \
  export --path=screenshot.png
```

## How it works

1. **Sidecar** builds the app from its Dockerfile using `directory.dockerBuild()`
2. **Sidecar** injects any provided environment variables
3. **Sidecar** starts the app as a service
4. **Sidecar** launches Playwright container that connects to the app service
5. Playwright navigates to the app and captures a screenshot

## Requirements for apps

Your app needs:
- A `Dockerfile` in the root directory
- The app should listen on the port specified by `$PORT` environment variable (default: 8000)
- The app should respond to HTTP requests on `/`

That's it! No Dagger module, no special build scripts, no app-specific environment variables in the sidecar.

## API

### `screenshot(appService, url?, port?, waitTime?)`

Low-level function to screenshot a running service.

**Parameters:**
- `appService` (Service) - Running web application service
- `url` (string) - URL path to navigate to (default: "/")
- `port` (number) - Port the service listens on (default: 8000)
- `waitTime` (number) - Time to wait for page load in ms (default: 5000)

**Returns:** File - Screenshot PNG

### `screenshotApp(appSource, envVars?, waitTime?, port?)`

High-level function to build from Dockerfile and screenshot.

**Parameters:**
- `appSource` (Directory) - Directory containing app source and Dockerfile
- `envVars` (string) - Comma-separated KEY=VALUE pairs (e.g., "PORT=8000,DEBUG=true")
- `waitTime` (number) - Time to wait for page load in ms (default: 20000)
- `port` (number) - Port the app listens on (default: 8000)

**Returns:** File - Screenshot PNG

## Files

- `.dagger/src/index.ts` - Main Dagger module with two functions
- `.dagger/playwright/` - Generic Playwright test setup
  - `screenshot.spec.ts` - Screenshot capture test
  - `playwright.config.ts` - Playwright configuration
  - `package.json` - Playwright dependencies
- `run-screenshot.sh` - Convenience wrapper script
- `dagger.json` - Dagger module configuration
