import { test, chromium } from "@playwright/test";
import { exec } from "child_process";
import { promisify } from "util";

const execAsync = promisify(exec);

test("capture app screenshot", async () => {
  // Get the IP address of the app container
  const { stdout } = await execAsync("getent hosts app | awk '{ print $1 }'");
  const appIp = stdout.trim();

  console.log(`Connecting to app at IP: ${appIp}`);

  const browser = await chromium.launch({
    args: ["--no-sandbox", "--disable-setuid-sandbox"],
  });

  const page = await browser.newPage();

  try {
    // navigate to the app using IP instead of hostname
    await page.goto(`http://${appIp}:8000/`, {
      waitUntil: "domcontentloaded",
      timeout: 30000,
    });

    // wait for 15 seconds
    await page.waitForTimeout(15000);

    // take screenshot and save to mounted volume
    await page.screenshot({
      path: "/screenshots/screenshot.png",
      fullPage: true,
    });
  } finally {
    await browser.close();
  }
});
