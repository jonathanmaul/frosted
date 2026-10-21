// Fails when UI copy uses words forbidden by docs/DESIGN.md §8.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

const FORBIDDEN = ["revolutionary", "seamless", "next-gen", "unlock", "empower", "cutting-edge"];
const ROOTS = ["app", "components/ui", "components/landing", "lib"];

const files = [];
const walk = (dir) => {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) walk(p);
    else if (/\.(tsx?|mdx?)$/.test(name)) files.push(p);
  }
};
ROOTS.forEach(walk);

const hits = [];
for (const f of files) {
  const text = readFileSync(f, "utf8").toLowerCase();
  for (const w of FORBIDDEN) if (new RegExp(`\\b${w}\\b`).test(text)) hits.push(`${f}: "${w}"`);
}
if (hits.length) {
  console.error("Forbidden copy words (docs/DESIGN.md §8):\n" + hits.join("\n"));
  process.exit(1);
}
console.log(`copy check ok (${files.length} files)`);
