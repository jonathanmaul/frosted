import { expect, test, type Page } from "@playwright/test";
import { apiGet, markets, mine, operator, shot } from "./stack";

const EVIDENCE = "https://example.org/statement/e2e";

/** The coordinator publishes on its next tick, so reload until the page shows the new state. */
async function seen(page: Page, check: () => Promise<void>) {
  await expect(async () => { await page.reload(); await check(); }).toPass({ timeout: 90_000, intervals: [2_000] });
}

test.describe.serial("a committee market, driven from the operator CLI and watched in the browser", () => {
  test("shows the void deadline while nobody has proposed", async ({ page }) => {
    const { committee } = await markets();
    await page.goto(`/markets/${committee.market_id}`);
    await expect(page.getByRole("heading", { name: "Committee decision" })).toBeVisible();
    await expect(page.getByText(/No proposal yet/)).toBeVisible();
    await expect(page.getByText(/voided at block/)).toBeVisible();
    await expect(page.getByText(/one person holds all three member keys/)).toBeVisible();
    await shot(page, "committee-waiting");
  });

  test("propose, confirm, withdraw, then resolve after the dispute window", async ({ page }) => {
    const { committee } = await markets();
    const def = (await apiGet<{ market_definition: { resolve_height: number } }>(`/markets/${committee.market_id}`)).market_definition;
    const tip = Number((await apiGet<{ height: number }>("/chain/tip")).height);
    if (tip <= def.resolve_height) mine(def.resolve_height - tip + 2);

    await page.goto(`/markets/${committee.market_id}`);
    operator("resolution", "propose", committee.market_id, "--outcome", "0", "--evidence-url", EVIDENCE, "--member", "member-1");
    await seen(page, async () => {
      await expect(page.getByText("member-1 · Proposed · YES")).toBeVisible({ timeout: 1_500 });
      await expect(page.getByRole("link", { name: EVIDENCE })).toBeVisible({ timeout: 1_500 });
    });

    operator("resolution", "confirm", committee.market_id, "--outcome", "0", "--evidence-url", EVIDENCE, "--member", "member-2");
    await seen(page, async () => {
      await expect(page.getByText("member-2 · Confirmed · YES")).toBeVisible({ timeout: 1_500 });
      await expect(page.getByText(/Dispute window ends at block \d[\d,]*: \d+ blocks? left/)).toBeVisible({ timeout: 1_500 });
    });
    await shot(page, "committee-proposed");

    operator("resolution", "withdraw", committee.market_id, "--member", "member-3");
    await seen(page, async () => { await expect(page.getByText(/No proposal yet/)).toBeVisible({ timeout: 1_500 }); });

    operator("resolution", "propose", committee.market_id, "--outcome", "0", "--evidence-url", EVIDENCE, "--member", "member-3");
    operator("resolution", "confirm", committee.market_id, "--outcome", "0", "--evidence-url", EVIDENCE, "--member", "member-1");
    mine(8);   // dispute_window_blocks is 6
    await expect.poll(async () => (await apiGet<{ status: string }>(`/markets/${committee.market_id}`)).status, { timeout: 90_000 }).toMatch(/RESOLVED|VOID/);
    await seen(page, async () => { await expect(page.getByText(/Outcome: (YES|VOID)/)).toBeVisible({ timeout: 1_500 }); });
    await shot(page, "committee-resolved");
  });

  test("the operator page shows both markets and the committee commands", async ({ page }) => {
    const { chain, committee } = await markets();
    await page.goto("/operator");
    await expect(page.getByText(chain.question)).toBeVisible();
    await expect(page.getByText(committee.question)).toBeVisible();
    await expect(page.getByText(/frosted-operator resolution propose/)).toBeVisible();
    await shot(page, "operator");
  });
});
