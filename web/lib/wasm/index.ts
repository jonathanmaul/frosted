export type BuiltBet = { memo_text: string; zip321_uri: string; bet_fingerprint: string; env_esk: string; bet_salt: string };
export type GlassWasm = {
  default: () => Promise<unknown>;
  build_bet: (input: unknown) => BuiltBet;
  parse_payout_ua: (ua: string, network: string) => { ok: boolean; receiver?: string; error?: string };
  new_persona: (registry: string) => { k_root: string; persona_id: string; frstp1_uri: string };
  receiver_commitment: (receipt: unknown) => string;
  envelope_proof: (receipt: unknown, noteRef: string) => unknown;
};

export async function loadGlassWasm(): Promise<GlassWasm> {
  const url = "/wasm/zcash_glass_wasm.js";
  const bindings: GlassWasm = await import(/* webpackIgnore: true */ /* @vite-ignore */ url);
  await bindings.default();
  return bindings;
}
