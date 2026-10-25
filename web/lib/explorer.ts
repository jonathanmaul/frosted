// Public block explorer links. Only set for a public network (testnet/mainnet); regtest has none.
const BASE = (process.env.NEXT_PUBLIC_EXPLORER_URL ?? "").replace(/\/+$/, "");

export const hasExplorer = BASE !== "";
export const explorerTxUrl = (txid: string) => (BASE ? `${BASE}/tx/${encodeURIComponent(txid)}` : null);
export const explorerBlockUrl = (height: number) => (BASE ? `${BASE}/block/${height}` : null);
