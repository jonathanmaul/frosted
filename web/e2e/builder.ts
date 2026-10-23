import { expect, type Page } from "@playwright/test";

/** Walks the four Bet Builder steps and returns the payment URI. */
export async function sealBet(page: Page, opts: { outcome: "YES" | "NO"; amount: string; payout: string; persona?: string }): Promise<string> {
  await page.getByRole("radio", { name: opts.outcome }).check();
  await page.getByRole("button", { name: "Continue" }).click();
  await page.getByLabel(/Amount in ZEC/).fill(opts.amount);
  await page.getByRole("button", { name: "Continue" }).click();
  await page.getByLabel(/Payout Unified Address/).fill(opts.payout);
  if (opts.persona) {
    await page.getByLabel("Tag this bet with my Persona").check();
    await page.getByLabel(/Persona passphrase/).fill(opts.persona);
  }
  await page.getByRole("button", { name: "Continue" }).click();
  await expect(page.getByText("04 / Review & seal")).toBeVisible();
  await page.getByRole("button", { name: "Seal & save receipt" }).click();
  await expect(page.getByText("Receipt saved on this device.")).toBeVisible();
  return page.getByLabel("Payment URI").inputValue();
}
