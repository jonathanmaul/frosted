// Drives the stack that `scripts/dev.sh` started: chain, wallets and the operator CLI.
import { execFileSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import path from "node:path";
import type { Page } from "@playwright/test";

const root = path.resolve(__dirname, "../..");
export const API = process.env.E2E_API_URL ?? "http://localhost:18080/v1";
export const WEB = process.env.E2E_WEB_URL ?? "http://localhost:13000";

const env = {
  ...process.env,
  DEV_STATE: process.env.E2E_STATE ?? ".regtest/e2e",
  API_PORT: process.env.E2E_API_PORT ?? "18080",
  COORD_PORT: process.env.E2E_COORD_PORT ?? "18090",
  WEB_PORT: process.env.E2E_WEB_PORT ?? "13000",
};

/** Runs `scripts/dev.sh <args>` and returns its output. */
export function dev(...args: string[]): string {
  return execFileSync(path.join(root, "scripts/dev.sh"), args, { env, encoding: "utf8", timeout: 200_000 }).trim();
}
export const pay = (uri: string) => dev("pay", uri);
export const mine = (n: number) => dev("mine", String(n));
export const operator = (...args: string[]) => dev("op", ...args);
export const recipientAddress = () => dev("recipient");

export type MarketRow = { market_id: string; question: string; status: string; resolution_type: string };
export async function apiGet<T>(p: string): Promise<T> {
  const res = await fetch(`${API}${p}`);
  if (!res.ok) throw new Error(`GET ${p}: ${res.status}`);
  return res.json() as Promise<T>;
}
export async function markets(): Promise<{ chain: MarketRow; committee: MarketRow }> {
  const { markets: rows } = await apiGet<{ markets: MarketRow[] }>("/markets");
  const chain = rows.find((m) => m.resolution_type !== "COMMITTEE");
  const committee = rows.find((m) => m.resolution_type === "COMMITTEE");
  if (!chain || !committee) throw new Error("the demo markets are missing: is the stack running (scripts/e2e.sh)?");
  return { chain, committee };
}

/** Full-page screenshot for human review, saved under test-results/shots. */
export async function shot(page: Page, name: string) {
  const dir = path.join(__dirname, "../test-results/shots");
  mkdirSync(dir, { recursive: true });
  await page.screenshot({ path: path.join(dir, `${name}.png`), fullPage: true });
}

/** Everything a page stored in IndexedDB, as one string, to prove what is (not) kept in plain text. */
export async function indexedDbDump(page: Page): Promise<string> {
  return page.evaluate(
    () =>
      new Promise<string>((resolve, reject) => {
        const open = indexedDB.open("frosted");
        open.onerror = () => reject(open.error);
        open.onsuccess = () => {
          const db = open.result;
          const names = Array.from(db.objectStoreNames);
          const out: Record<string, unknown[]> = {};
          if (!names.length) return resolve("{}");
          const tx = db.transaction(names, "readonly");
          let pending = names.length;
          for (const n of names) {
            const all = tx.objectStore(n).getAll();
            all.onsuccess = () => {
              out[n] = all.result;
              if (--pending === 0) { db.close(); resolve(JSON.stringify(out)); }
            };
          }
        };
      }),
  );
}

/** The app's own error message (Next also renders an empty role=alert route announcer). */
export const alert = (page: Page) => page.locator('[role="alert"]:not(#__next-route-announcer__)');
