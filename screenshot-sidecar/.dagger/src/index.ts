/**
 * Screenshot sidecar for web applications
 *
 * A reusable Dagger module that captures screenshots of running web applications
 * using Playwright. Assumes the app has a Dockerfile.
 */
import { dag, Directory, Service, object, func } from "@dagger.io/dagger"

@object()
export class ScreenshotSidecar {
  /**
   * Capture a screenshot of a running web service
   *
   * @param appService The running web application service to screenshot
   * @param url The URL path to navigate to (default: "/")
   * @param port The port the service is listening on (default: 8000)
   * @param waitTime Time to wait for page to load in ms (default: 5000)
   * @returns Directory containing screenshot.png and logs.txt
   */
  @func()
  async screenshot(
    appService: Service,
    url?: string,
    port?: number,
    waitTime?: number
  ): Promise<Directory> {
    const targetUrl = url || "/"
    const targetPort = port || 8000
    const wait = waitTime || 5000

    // timestamp for cache busting
    const timestamp = Date.now().toString()

    // load playwright source from module root (source is now "." in dagger.json)
    const playwrightSource = dag.currentModule().source().directory("playwright")

    const playwrightContainer = dag
      .container()
      .from("mcr.microsoft.com/playwright:v1.40.0-jammy")
      .withWorkdir("/tests")
      .withDirectory("/tests", playwrightSource, {
        exclude: ["node_modules"]
      })
      .withExec(["npm", "install"])
      .withExec(["npx", "playwright", "install", "chromium"])
      .withServiceBinding("app", appService)
      .withEnvVariable("TARGET_URL", targetUrl)
      .withEnvVariable("TARGET_PORT", targetPort.toString())
      .withEnvVariable("WAIT_TIME", wait.toString())
      .withEnvVariable("CACHE_BUST", timestamp)
      .withExec(["npx", "playwright", "test"])

    return playwrightContainer.directory("/screenshots")
  }

  /**
   * Build and screenshot an app from a directory with a Dockerfile
   *
   * @param appSource Directory containing the app source and Dockerfile
   * @param envVars Optional environment variables as comma-separated KEY=VALUE pairs (e.g., "PORT=8000,DEBUG=true")
   * @param waitTime Time to wait for page to load in ms (default: 20000)
   * @param port Port the app listens on (default: 8000)
   * @returns Directory containing screenshot.png and logs.txt
   */
  @func()
  async screenshotApp(
    appSource: Directory,
    envVars?: string,
    waitTime?: number,
    port?: number
  ): Promise<Directory> {
    const targetPort = port || 8000

    // build container from Dockerfile
    let appContainer = appSource.dockerBuild()

    // parse and apply environment variables
    if (envVars) {
      const pairs = envVars.split(",")
      for (const pair of pairs) {
        const [key, value] = pair.split("=")
        if (key && value) {
          appContainer = appContainer.withEnvVariable(key.trim(), value.trim())
        }
      }
    }

    const appService = appContainer.withExposedPort(targetPort).asService()

    return this.screenshot(appService, "/", targetPort, waitTime || 20000)
  }
}
