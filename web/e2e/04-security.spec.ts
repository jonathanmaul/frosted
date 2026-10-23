import { expect, test } from "@playwright/test";
import { API, markets } from "./stack";

test.describe("browser-enforced security", () => {
  test("every page sends the security headers and a CSP that allows only this origin and the API", async ({ request }) => {
    const { chain } = await markets();
    for (const path of ["/", "/markets", `/markets/${chain.market_id}`, "/persona", "/thaw", "/verify/" + chain.market_id]) {
      const res = await request.get(path);
      const h = res.headers();
      expect(h["x-frame-options"], path).toBe("DENY");
      expect(h["x-content-type-options"], path).toBe("nosniff");
      expect(h["referrer-policy"], path).toBe("no-referrer");
      expect(h["permissions-policy"], path).toContain("camera=()");
      expect(h["x-powered-by"], path).toBeUndefined();
      const csp = h["content-security-policy"];
      expect(csp, path).toContain("default-src 'self'");
      expect(csp, path).toContain("frame-ancestors 'none'");
      expect(csp, path).toContain("object-src 'none'");
      expect(csp, path).toContain(`connect-src 'self' ${new URL(API).origin}`);
      expect(csp, path).not.toMatch(/https?:\/\/(?!localhost)/);   // no third-party origin anywhere
    }
  });

  test("normal use triggers no CSP violation, including the WASM Bet Builder, the 3D landing and live updates", async ({ page }) => {
    const { chain } = await markets();
    const violations: string[] = [];
    page.on("console", (m) => { if (/content security policy/i.test(m.text())) violations.push(m.text()); });
    await page.addInitScript(() => document.addEventListener("securitypolicyviolation", (e) => console.error(`Content Security Policy: ${e.violatedDirective} ${e.blockedURI}`)));
    for (const path of ["/", "/markets", `/markets/${chain.market_id}`, "/persona", "/leaderboard", "/my-bets", "/how", "/operator", `/verify/${chain.market_id}`]) {
      await page.goto(path);
      await page.waitForLoadState("networkidle");
    }
    // Exercise WASM: build a bet in the browser.
    await page.goto(`/markets/${chain.market_id}`);
    await page.getByRole("button", { name: "Continue" }).click();
    await page.getByRole("button", { name: "Continue" }).click();
    await page.getByLabel(/Payout Unified Address/).fill("uregtest1notvalid");
    await page.getByRole("button", { name: "Continue" }).click();
    await expect(page.locator('[role="alert"]:not(#__next-route-announcer__)')).toBeVisible();   // WASM ran and rejected it
    expect(violations).toEqual([]);
  });

  test("the browser blocks a script from another origin and the page cannot be framed", async ({ page, browser }) => {
    await page.goto("/markets");
    const blocked = await page.evaluate(
      () => new Promise<string>((resolve) => {
        document.addEventListener("securitypolicyviolation", (e) => resolve(`${e.violatedDirective} ${e.blockedURI}`));
        const s = document.createElement("script");
        s.src = "https://example.org/evil.js";
        document.head.appendChild(s);
        setTimeout(() => resolve("not blocked"), 5_000);
      }),
    );
    expect(blocked).toContain("script-src");
    const fetched = await page.evaluate(() => fetch("https://example.org/", { mode: "no-cors" }).then(() => "sent", () => "blocked"));
    expect(fetched).toBe("blocked");

    // A different origin cannot embed the app.
    const context = await browser.newContext();
    const outer = await context.newPage();
    await outer.route("http://framer.test/", (r) => r.fulfill({ contentType: "text/html", body: `<iframe id="f" src="${page.url()}"></iframe>` }));
    await outer.goto("http://framer.test/");
    await outer.waitForTimeout(2_000);
    const frame = outer.frames().find((f) => f.url().startsWith(page.url().split("/markets")[0]));
    expect(frame === undefined || (await frame.title().catch(() => "")) === "").toBe(true);
    await context.close();
  });
});
