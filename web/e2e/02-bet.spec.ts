import { expect, test } from "@playwright/test";
import { sealBet } from "./builder";
import { alert, apiGet, indexedDbDump, markets, pay, recipientAddress, shot } from "./stack";

const poolZat = async (id: string) => (await apiGet<{ pool_total_zat: number }>(`/markets/${id}/odds`)).pool_total_zat;
const zec = (zat: number) => (zat / 1e8).toFixed(2);

test.describe("a real bet, built in the browser and paid on the chain", () => {
  test("seals a bet with WASM, keeps the receipt locally, and is counted after payment", async ({ page }) => {
    const { chain } = await markets();
    const payout = recipientAddress();
    const apiCalls: string[] = [];
    page.on("request", (r) => { if (r.method() !== "GET") apiCalls.push(`${r.method()} ${r.url()}`); });

    const before = await poolZat(chain.market_id);
    await page.goto(`/markets/${chain.market_id}`);
    await expect(page.getByText("LIVE")).toBeVisible();
    const uri = await sealBet(page, { outcome: "YES", amount: "0.02", payout });
    await shot(page, "bet-sealed");

    // The QR and the URI exist; the payout address and the secrets stayed in this browser.
    await expect(page.getByRole("img", { name: /ZIP-321 payment QR/ })).toBeVisible();
    expect(uri.startsWith("zcash:")).toBe(true);
    expect(uri).toContain("amount=0.02");
    expect(apiCalls, "sealing a bet must not send anything to a server").toEqual([]);
    const stored = await indexedDbDump(page);
    expect(stored).toContain(payout);          // the receipt keeps the payout address locally (My Bets needs it)
    expect(stored).toContain("env_esk");

    // Pay from the faucet wallet; the market page updates without a reload (SSE).
    pay(uri);
    // The page must show the pool GROWING by this bet, live, without a reload.
    await expect(page.getByText(`${zec(before + 2_000_000)} ZEC in the pool.`)).toBeVisible({ timeout: 150_000 });
    expect(await poolZat(chain.market_id)).toBe(before + 2_000_000);
    await shot(page, "bet-counted-market");

    // My Bets matches the receipt to the public note.
    await page.goto("/my-bets");
    // The indexer promotes the bet once it has enough confirmations; refresh until it shows.
    await expect(async () => {
      await page.getByRole("button", { name: "Refresh statuses" }).click();
      await expect(page.getByText("Counted", { exact: true }).first()).toBeVisible({ timeout: 4_000 });
    }).toPass({ timeout: 90_000 });
    await expect(page.getByText(/Your bet is counted in the pool\./)).toBeVisible();
    await shot(page, "my-bets-counted");
  });

  test("refuses an invalid payout address before any QR exists", async ({ page }) => {
    const { chain } = await markets();
    await page.goto(`/markets/${chain.market_id}`);
    await page.getByRole("button", { name: "Continue" }).click();
    await page.getByRole("button", { name: "Continue" }).click();
    await page.getByLabel(/Payout Unified Address/).fill("not-an-address");
    await page.getByRole("button", { name: "Continue" }).click();
    await expect(alert(page)).toBeVisible();
    await expect(page.getByRole("img", { name: /ZIP-321/ })).toHaveCount(0);
  });
});
