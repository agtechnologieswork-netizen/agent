/**
 * Screenshot capture module for tRPC template applications
 *
 * Builds the application and captures a full-page screenshot using Playwright
 */
import { dag, Container, Directory, File, object, func } from "@dagger.io/dagger"

@object()
export class Screenshot {
  /**
   * Build the client application (React + Vite)
   */
  @func()
  buildClient(source: Directory): Directory {
    return dag
      .container()
      .from("node:20-alpine")
      .withMountedDirectory("/app/client", source.directory("client"))
      .withMountedDirectory("/app/server", source.directory("server"))
      .withWorkdir("/app/client")
      .withExec(["npm", "install"])
      .withExec(["npm", "run", "build"])
      .directory("/app/client/dist")
  }

  /**
   * Build the production application container
   */
  @func()
  buildApp(source: Directory): Container {
    const clientDist = this.buildClient(source)

    return dag
      .container()
      .from("node:20-alpine")
      .withExec(["apk", "add", "--no-cache", "curl"])
      .withMountedDirectory("/app/server", source.directory("server"))
      .withWorkdir("/app/server")
      .withExec(["npm", "install"])
      .withDirectory("/app/server/public", clientDist)
      .withExposedPort(8000)
      .withEntrypoint(["npm", "start"])
  }

  /**
   * Capture a screenshot of the running application
   */
  @func()
  async screenshot(
    source: Directory,
    databricksHost?: string,
    databricksToken?: string
  ): Promise<File> {
    // build and start the app as a service
    let appContainer = this.buildApp(source)
      .withEnvVariable("PORT", "8000")
      .withEnvVariable("CORS_ORIGINS", "*")

    if (databricksHost) {
      appContainer = appContainer.withEnvVariable("DATABRICKS_HOST", databricksHost)
    }
    if (databricksToken) {
      appContainer = appContainer.withEnvVariable("DATABRICKS_TOKEN", databricksToken)
    }

    // bust cache by adding timestamp
    const timestamp = Date.now().toString()

    const appService = appContainer.asService()

    // create playwright container with cache busting
    const playwrightContainer = dag
      .container()
      .from("mcr.microsoft.com/playwright:v1.40.0-jammy")
      .withWorkdir("/tests")
      .withDirectory("/tests", source.directory("tests/playwright"), {
        exclude: ["node_modules"]
      })
      .withExec(["npm", "install"])
      .withExec(["npx", "playwright", "install", "chromium"])
      .withServiceBinding("app", appService)
      .withEnvVariable("CACHE_BUST", timestamp)
      .withExec(["npx", "playwright", "test"])

    // return the screenshot file
    return playwrightContainer.file("/screenshots/screenshot.png")
  }
}
