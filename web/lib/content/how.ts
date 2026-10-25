// S-12 content. Leaked-metadata items mirror docs/PROJECT_SPEC_FROSTED.md §24.3 one to one;
// tests/how.test.tsx checks the count so the page cannot silently drop an item.
export const PUBLIC_ITEMS = [
  "Every bet's amount, outcome, and block height.",
  "Every bet's memo, including the Thaw Tag and the sealed envelope ciphertext.",
  "Pools and odds per outcome, recomputable by anyone with the viewing key.",
  "Resolution evidence and the public payout plan (kind and amount per bet).",
];

export const PRIVATE_ITEMS = [
  "Who sent a bet: no sender address exists on the shielded chain.",
  "Bet transaction ids are not shown in this app.",
  "Payout addresses are sealed to the coordinator and never shown publicly.",
  "A Persona's bets stay unlinkable until its owner chooses to thaw.",
];

export const TRUST_LIMITS = [
  "Trust-minimized, not trustless: the FROST committee cannot lie about a Chain-Verified result, but could withhold funds, because Zcash L1 has no payout script.",
  "The Payout Coordinator opens sealed envelopes, so it knows each bettor's payout address (not their identity). Use a fresh address for payouts.",
  "In this deployment one person holds all three FROST shares and the coordinator key. 2-of-3 is shown technically, not as separation of trust.",
  "Thaw does not prevent Sybil: one person can run many Personas.",
];

export const LEAKED_METADATA = [
  "For every bet: amount, outcome, block height, and memo (including the Thaw Tag and the envelope ciphertext) are public to anyone holding the UIVK. That is the design.",
  "The existence of a shielded transaction in a block (not its content) is visible to anyone scanning the chain.",
  "The Payout Coordinator knows the payout address of every bettor.",
  "If full audit (F-13) is enabled, payout addresses and amounts become public after settlement.",
  "After a Thaw, all bets tagged by that Persona become public and linked.",
  "The public payout plan reveals the kind and amount of each payout per bet (note_ref), not the recipient address.",
  "The Treasury address and every sweep to it are public.",
  "An Envelope Proof published by a bettor reveals that bet's payout receiver.",
  "After settlement, the escrow still holds uneconomic dust and notes that arrived after the plan cutoff height.",
  "The API server sees HTTP requests. Use Tor if that matters to you.",
];

export const LEGAL =
  "Frosted is a research project, not a betting service. Do not bet money you are not prepared to lose, and check the law where you live.";

/** Zcash technology Frosted actually uses (see Cargo.toml and scripts/testnet.sh). Text and links only, no third-party logos. */
export const BUILT_ON = [
  { name: "librustzcash", role: "Transactions, keys, note decryption and the hot wallet (pre-release crates for NU7).", href: "https://github.com/zcash/librustzcash" },
  { name: "Zebra", role: "The node the indexer, API and verifier read the chain from (JSON-RPC).", href: "https://github.com/ZcashFoundation/zebra" },
  { name: "Orchard · Ironwood", role: "The shielded pools escrow notes live in; amounts and senders stay encrypted.", href: "https://github.com/zcash/orchard" },
  { name: "FROST", role: "Zcash Foundation threshold signatures: two of three members sign one payout.", href: "https://github.com/ZcashFoundation/frost" },
  { name: "ZIP-321", role: "The payment request behind the Bet Builder's QR code and URI.", href: "https://zips.z.cash/zip-0321" },
  { name: "Zcash testnet (NU7)", role: "Where every Frosted transaction is mined; check them on a public explorer.", href: "https://testnet.zecblock.com" },
];
