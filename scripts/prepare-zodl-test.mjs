// M6 manual wallet fixture. Testnet only; this script never broadcasts a payment.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";

const root = new URL("../", import.meta.url);
const state = new URL(".regtest/m6-zodl/", root);
const require = createRequire(new URL("web/package.json", root));
const qr = require("qrcode");
const wasm = await import(new URL("web/lib/wasm/pkg/zcash_glass_wasm.js", root));
await wasm.default({ module_or_path: await readFile(new URL("web/lib/wasm/pkg/zcash_glass_wasm_bg.wasm", root)) });

// Public test receiver from SPIKES M0.2/M0.6; never use this fixture on mainnet.
const ua = "utest10g3wfuycgmt6pts02urpmv3sfx95q4ak39gdu6ygh8ey3sqf9kcluthm67hfmu0ct3w50h9u68hw3hmtwh4urnj9kf037rv60qmrjaqu";
const { market_definition: market } = JSON.parse(await readFile(new URL("docs/test-vectors/hash.json", root)));
const envelope = JSON.parse(await readFile(new URL("docs/test-vectors/envelope.json", root)));
market.network = "testnet";
market.envelope_pk = Buffer.from(envelope.envelope_pk, "hex").toString("base64url");
market.treasury_address = ua;
for (const outcome of market.outcomes) outcome.address = ua;
const input = { market_definition: market, outcome_index: 0, amount_zat: 1_000_000, payout_ua: ua, k_root: null };
const built = wasm.build_bet(input);
assert.equal(Buffer.byteLength(built.memo_text), 237);
assert.equal(new URL(built.zip321_uri).searchParams.get("amount"), "0.01000000");
assert.equal(Buffer.from(new URL(built.zip321_uri).searchParams.get("memo"), "base64url").toString(), built.memo_text);
await mkdir(state, { recursive: true, mode: 0o700 });
await writeFile(new URL("input.json", state), JSON.stringify(input, null, 2), { mode: 0o600 });
await writeFile(new URL("built.json", state), JSON.stringify(built, null, 2), { mode: 0o600 });
await writeFile(new URL("memo.txt", state), built.memo_text, { mode: 0o600 });
await writeFile(new URL("payment-uri.txt", state), built.zip321_uri, { mode: 0o600 });
await qr.toFile(new URL("payment-qr.png", state).pathname, built.zip321_uri, { errorCorrectionLevel: "M", margin: 4, scale: 8 });
const digest = createHash("sha256").update(built.memo_text).digest("hex");
console.log(`Prepared TESTNET fixture: .regtest/m6-zodl/payment-qr.png; 0.01 TAZ; memo 237 bytes; SHA256 ${digest}`);
