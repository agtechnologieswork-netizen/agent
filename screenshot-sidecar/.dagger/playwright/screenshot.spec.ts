import { test, chromium } from "@playwright/test";
import { mkdir } from "fs/promises";

test("capture app screenshot", async () => {
  // ensure screenshots directory exists
  await mkdir("/screenshots", { recursive: true });

  const targetUrl = process.env.TARGET_URL || "/";
  const targetPort = process.env.TARGET_PORT || "8000";
  const waitTime = parseInt(process.env.WAIT_TIME || "5000");

  console.log(`Navigating to http://app:${targetPort}${targetUrl}`);

  const browser = await chromium.launch({
    args: ["--no-sandbox", "--disable-setuid-sandbox"],
  });

  const page = await browser.newPage();

  try {
    await page.goto(`http://app:${targetPort}${targetUrl}`, {
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
