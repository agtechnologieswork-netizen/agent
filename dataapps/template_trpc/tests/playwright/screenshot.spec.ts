import { test, chromium } from "@playwright/test";
import { mkdir } from "fs/promises";
import { exec } from "child_process";
import { promisify } from "util";

const execAsync = promisify(exec);

test("capture app screenshot", async () => {
  // ensure screenshots directory exists
  await mkdir("/screenshots", { recursive: true });

  // resolve hostname to IP to avoid SSL protocol errors
  const { stdout } = await execAsync("getent hosts app | awk '{ print $1 }'");
  const appIp = stdout.trim();

  console.log(`Connecting to app at IP: ${appIp}`);

  const browser = await chromium.launch({
    args: ["--no-sandbox", "--disable-setuid-sandbox"],
  });

  const page = await browser.newPage();

  try {
    // navigate using IP instead of hostname to avoid SSL protocol errors
    await page.goto(`http://${appIp}:8000/`, {
      waitUntil: "domcontentloaded",
      timeout: 30000,
    });

    // wait for app to fully load
    await page.waitForTimeout(20000);

    // take screenshot and save to /screenshots
    await page.screenshot({
      path: "/screenshots/screenshot.png",
      fullPage: true,
    });

    console.log("Screenshot saved to /screenshots/screenshot.png");
  } finally {
    await browser.close();
  }
});
