import { test, chromium } from "@playwright/test";
import { mkdir } from "fs/promises";
import { exec } from "child_process";
import { promisify } from "util";

const execAsync = promisify(exec);

test("capture app screenshot", async () => {
  // ensure screenshots directory exists
  await mkdir("/screenshots", { recursive: true });

  const targetUrl = process.env.TARGET_URL || "/";
  const targetPort = process.env.TARGET_PORT || "8000";
  const waitTime = parseInt(process.env.WAIT_TIME || "5000");

  // resolve hostname to IP to avoid SSL protocol errors with service binding
  const { stdout } = await execAsync("getent hosts app | awk '{ print $1 }'");
  const appIp = stdout.trim();

  console.log(`Resolved app to IP: ${appIp}`);
  console.log(`Navigating to http://${appIp}:${targetPort}${targetUrl}`);

  const browser = await chromium.launch({
    args: ["--no-sandbox", "--disable-setuid-sandbox"],
  });

  const page = await browser.newPage();

  try {
    // use IP instead of hostname to avoid SSL protocol errors
    await page.goto(`http://${appIp}:${targetPort}${targetUrl}`, {
      waitUntil: "domcontentloaded",
      timeout: 30000,
    });

    // wait for app to load
    await page.waitForTimeout(waitTime);

    // take full page screenshot
    await page.screenshot({
      path: "/screenshots/screenshot.png",
      fullPage: true,
    });

    console.log("Screenshot saved to /screenshots/screenshot.png");
  } finally {
    await browser.close();
  }
});
