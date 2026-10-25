// Captures the public API responses the web app reads, for a deployment without a live backend
// (NEXT_PUBLIC_SNAPSHOT_MODE=true). Every value comes from the real API; nothing is invented.
//   node scripts/snapshot.mjs <api base, e.g. http://127.0.0.1:28080/v1> <market_id> [market_id...]
// Usually run through scripts/export-snapshot.sh from the repo root.
import { writeFileSync } from "node:fs";

const [api, ...ids] = process.argv.slice(2);
if (!api || !ids.length) { console.error("usage: node scripts/snapshot.mjs <api> <market_id>..."); process.exit(2); }

const get = async (path) => {
  const res = await fetch(api + path);
  if (res.status === 404) return null;
  if (!res.ok) throw new Error(`${path}: HTTP ${res.status}`);
  return res.json();
};

const data = {};
const list = await get("/markets");
data["/markets"] = { markets: list.markets.filter((m) => ids.includes(m.market_id)) };
data["/chain/tip"] = await get("/chain/tip");
data["/leaderboard"] = await get("/leaderboard");
data["/personas/registry"] = await get("/personas/registry");
for (const id of ids) {
  const base = `/markets/${encodeURIComponent(id)}`;
  for (const path of [base, `${base}/odds`, `${base}/bets?limit=500`, `${base}/resolution`, `${base}/payout`]) data[path] = await get(path);
}
const out = { captured_at: new Date().toISOString(), tip: data["/chain/tip"]?.height ?? null, responses: data };
writeFileSync("lib/snapshot.json", JSON.stringify(out, null, 1) + "\n");
console.log(`wrote lib/snapshot.json (${Object.keys(data).length} responses, tip ${out.tip})`);
