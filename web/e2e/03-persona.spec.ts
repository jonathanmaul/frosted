import { expect, test, type BrowserContext, type Page } from "@playwright/test";
import { sealBet } from "./builder";
import { alert, indexedDbDump, markets, pay, recipientAddress, shot } from "./stack";

const PASSPHRASE = "correct horse battery staple";

test.describe.serial("a Persona, from creation to Thaw, in a real browser", () => {
  let backupKey = "";
  let personaId = "";
  // One browser profile for the whole flow: the encrypted Persona lives in this profile's IndexedDB.
  let context: BrowserContext;
  let page: Page;
  test.beforeAll(async ({ browser }) => { context = await browser.newContext(); page = await context.newPage(); });
  test.afterAll(async () => { await context.close(); });

  test("creates a Persona: no QR until the backup is confirmed, key stored only encrypted", async () => {
    await page.goto("/persona");
    await page.getByRole("button", { name: "Create Persona" }).click();
    backupKey = (await page.getByLabel("Backup key").textContent())!.trim();
    expect(backupKey).toMatch(/^[0-9a-f]{64}$/);
    await expect(page.getByLabel("Registration URI")).toHaveCount(0);
    await expect(page.getByRole("img", { name: /registration QR/ })).toHaveCount(0);

    // Until the backup is confirmed, nothing can be stored: the passphrase fields and the button are locked.
    await expect(page.locator("#new-passphrase")).toBeDisabled();
    await expect(page.getByRole("button", { name: "Encrypt and continue" })).toBeDisabled();
    await page.getByLabel(/I saved this key/).check();
    await page.locator("#new-passphrase").fill(PASSPHRASE);
    await page.locator("#confirm-passphrase").fill(PASSPHRASE + "x");
    await page.getByRole("button", { name: "Encrypt and continue" }).click();
    await expect(alert(page)).toContainText("do not match");
    await expect(page.getByLabel("Registration URI")).toHaveCount(0);
    await page.locator("#confirm-passphrase").fill(PASSPHRASE);
    await page.getByRole("button", { name: "Encrypt and continue" }).click();
    await expect(page.getByLabel("Registration URI")).toBeVisible();
    await expect(page.getByRole("img", { name: /registration QR/ })).toBeVisible();
    await shot(page, "persona-created");

    const stored = await indexedDbDump(page);
    expect(stored, "the backup key must never be stored in plain text").not.toContain(backupKey);
    expect(stored).toContain("PBKDF2-SHA256");
    personaId = JSON.parse(stored).personas[0].persona_id;
    expect(personaId).toMatch(/^[0-9a-f]{32}$/);

    // Survives a reload, and the wrong passphrase reveals nothing.
    await page.reload();
    await page.locator("#reveal-passphrase").fill("not the passphrase");
    await page.getByRole("button", { name: "Show backup key" }).click();
    await expect(alert(page)).toContainText("Wrong passphrase");
    await expect(page.getByLabel("Backup key")).toHaveCount(0);
    await page.locator("#reveal-passphrase").fill(PASSPHRASE);
    await page.getByRole("button", { name: "Show backup key" }).click();
    await expect(page.getByLabel("Backup key")).toHaveText(backupKey);
  });

  test("registers on the chain after the registration payment", async () => {
    await page.goto("/persona");
    const uri = await page.getByLabel("Registration URI").inputValue();
    pay(uri);
    await expect(page.getByText(/Registered at block/)).toBeVisible({ timeout: 150_000 });
    await expect(page.getByLabel("Registration URI")).toHaveCount(0);
    await shot(page, "persona-registered");
  });

  test("tags a bet with the Persona and the tag reaches the public bet", async () => {
    const { chain } = await markets();
    await page.goto(`/markets/${chain.market_id}`);
    const uri = await sealBet(page, { outcome: "NO", amount: "0.01", payout: recipientAddress(), persona: PASSPHRASE });
    pay(uri);
    await expect.poll(async () => {
      const res = await fetch(`${process.env.E2E_API_URL ?? "http://localhost:18080/v1"}/markets/${chain.market_id}/bets`);
      const bets = (await res.json()).bets as { tagged: boolean; amount_zat: number }[];
      return bets.some((b) => b.tagged && b.amount_zat === 1_000_000);
    }, { timeout: 120_000 }).toBe(true);
    const stored = JSON.parse(await indexedDbDump(page));
    expect(stored.bet_receipts.some((r: { tagged: boolean }) => r.tagged)).toBe(true);
    expect(JSON.stringify(stored)).not.toContain(backupKey);   // the root was only ever in memory
  });

  test("thaw needs the alias twice, then publishes the record", async () => {
    const { chain } = await markets();
    await page.goto("/thaw");
    await expect(page.getByText(/cannot be undone/)).toBeVisible();
    const thaw = page.getByRole("button", { name: "Thaw permanently" });
    await page.getByLabel("Public alias").fill("playwright");
    await page.getByLabel("Type the alias again to confirm").fill("playwrigh");
    await page.getByLabel("Passphrase").fill(PASSPHRASE);
    await expect(thaw).toBeDisabled();
    await page.getByLabel("Type the alias again to confirm").fill("playwright");
    await shot(page, "thaw-ready");
    await expect(thaw).toBeEnabled();
    await thaw.click();

    await expect(page.getByRole("heading", { name: "Thawed: playwright." })).toBeVisible();
    await expect(page.getByRole("table")).toContainText(chain.question);
    await expect(page.getByRole("table")).toContainText("Pending");   // the market is not settled yet
    await shot(page, "thaw-report");

    // The root is public now; the profile and leaderboard show the Persona.
    await page.goto(`/p/${personaId}`);
    await expect(page.getByRole("heading", { name: "playwright." })).toBeVisible();
    await expect(page.getByRole("table")).toContainText(chain.question);
    await shot(page, "persona-profile");
    await page.goto("/leaderboard");
    await expect(page.getByRole("link", { name: "playwright" })).toBeVisible();
    await shot(page, "leaderboard");
  });
});
