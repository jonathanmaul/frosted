import { expect, test } from "@playwright/test";
import { markets, shot } from "./stack";

test("the Verify page lists the CLI commands and runs the server-side check", async ({ page }) => {
  const { chain } = await markets();
  await page.goto(`/verify/${chain.market_id}`);
  await expect(page.getByText(`frosted-verify all ${chain.market_id} --node`)).toBeVisible();
  await expect(page.getByText(/frosted-verify envelope-proof proof\.json/)).toBeVisible();
  await page.getByRole("button", { name: "Run server-side check" }).click();
  await expect(page.getByText(/Result:/)).toBeVisible({ timeout: 180_000 });
  await expect(page.getByText("[FAIL]")).toHaveCount(0);
  await expect(page.getByText("[PASS]").first()).toBeVisible();
  await expect(page.getByText(/dishonest server could print PASS/).first()).toBeVisible();
  await shot(page, "verify");
});
