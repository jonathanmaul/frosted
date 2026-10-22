import { execFileSync } from "node:child_process";
import { cpSync, mkdirSync, readFileSync } from "node:fs";
import { gzipSync } from "node:zlib";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
execFileSync("wasm-pack", ["build", "crates/zcash-glass-wasm", "--target", "web", "--out-dir", "../../web/lib/wasm/pkg", "--release"], { cwd: root, stdio: "inherit" });
const pkg = new URL("../lib/wasm/pkg/", import.meta.url);
const publicDir = new URL("../public/wasm/", import.meta.url);
mkdirSync(publicDir, { recursive: true });
for (const name of ["zcash_glass_wasm.js", "zcash_glass_wasm_bg.wasm"]) {
  cpSync(new URL(name, pkg), new URL(name, publicDir));
}
const wasm = readFileSync(new URL("zcash_glass_wasm_bg.wasm", pkg));
const compressed = gzipSync(wasm).length;
if (compressed >= 1_000_000) throw new Error(`WASM gzip exceeds 1 MB: ${compressed}`);
console.log(`WASM: ${wasm.length} bytes; gzip: ${compressed} bytes`);
