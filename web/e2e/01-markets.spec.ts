import { expect, test } from "@playwright/test";
import { API, markets, shot, WEB } from "./stack";

test.describe("public pages in a real browser", () => {
  test("lists the demo markets and filters them with the custom Select", async ({ page }) => {
    const { chain, committee } = await markets();
    await page.goto("/markets");
    await expect(page.getByRole("heading", { name: committee.question })).toBeVisible();
    await expect(page.getByRole("heading", { name: chain.question })).toBeVisible();
    await shot(page, "markets");

    // No native <select> anywhere: the dropdown is our own listbox.
    expect(await page.locator("select").count()).toBe(0);
    const kind = page.getByRole("combobox", { name: "Kind" });
    await kind.click();
    await expect(page.getByRole("listbox")).toBeVisible();
    await shot(page, "markets-select-open");
    await page.getByRole("option", { name: "Committee" }).click();
    await expect(page.getByRole("listbox")).toHaveCount(0);
    await expect(page.getByRole("heading", { name: committee.question })).toBeVisible();
    await expect(page.getByRole("heading", { name: chain.question })).toHaveCount(0);

    // Keyboard: open with ArrowDown, move, choose with Enter; Escape leaves the value alone.
    await kind.focus();
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Home");
    await page.keyboard.press("Enter");
    await expect(kind).toContainText("All kinds");
    await expect(page.getByRole("heading", { name: chain.question })).toBeVisible();
    await kind.focus();
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Escape");
    await expect(page.getByRole("listbox")).toHaveCount(0);
    await expect(kind).toContainText("All kinds");

    // Clicking outside closes an open list.
    await page.getByRole("combobox", { name: "Status" }).click();
    await expect(page.getByRole("listbox")).toBeVisible();
    await page.getByRole("heading", { name: "The ledger is open." }).click();
    await expect(page.getByRole("listbox")).toHaveCount(0);
  });

  test("makes no request outside the web app and the API, and throws no page error", async ({ page }) => {
    const { chain } = await markets();
    const allowed = new Set([new URL(WEB).origin, new URL(API).origin]);
    const outsiders: string[] = [], errors: string[] = [];
    page.on("request", (r) => { if (!r.url().startsWith("data:") && !r.url().startsWith("blob:") && !allowed.has(new URL(r.url()).origin)) outsiders.push(r.url()); });
    page.on("pageerror", (e) => errors.push(e.message));
    for (const path of ["/", "/markets", `/markets/${chain.market_id}`, "/persona", "/leaderboard", "/my-bets", "/how", `/verify/${chain.market_id}`, "/operator"]) {
      await page.goto(path);
      await page.waitForLoadState("networkidle");
    }
    expect(outsiders).toEqual([]);
    expect(errors).toEqual([]);
  });

  test("works on a phone-sized screen without sideways scrolling", async ({ browser }) => {
    const { chain } = await markets();
    const context = await browser.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, isMobile: true });
    const page = await context.newPage();
    for (const [name, path] of [["markets", "/markets"], ["market", `/markets/${chain.market_id}`], ["persona", "/persona"], ["leaderboard", "/leaderboard"]] as const) {
      await page.goto(path);
      await page.waitForLoadState("networkidle");
      const overflow = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
      expect(overflow, `${path} overflows by ${overflow}px`).toBeLessThanOrEqual(1);
      await shot(page, `mobile-${name}`);
    }
    await context.close();
  });

  test("shows the market page with live odds, the Bet Builder and the verify link", async ({ page }) => {
    const { chain } = await markets();
    await page.goto(`/markets/${chain.market_id}`);
    await expect(page.getByRole("heading", { name: chain.question })).toBeVisible();
    await expect(page.getByText("LIVE")).toBeVisible();   // the SSE stream connected
    await expect(page.getByText("Seal your position.")).toBeVisible();
    await expect(page.getByRole("link", { name: /Verify this market yourself/ })).toBeVisible();
    await shot(page, "market-detail");
  });
});
