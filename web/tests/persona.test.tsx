import "fake-indexeddb/auto";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { BetBuilder } from "@/components/bettor/BetBuilder";
import { LeaderboardTable } from "@/components/persona/LeaderboardTable";
import { PersonaPanel } from "@/components/persona/PersonaPanel";
import { ThawPanel } from "@/components/persona/ThawPanel";
import { ThawReportView } from "@/components/persona/ThawReportView";
import { decryptRoot, encryptRoot, formatRoi, getPersona, hasBadge, rootToBase64Url, savePersona, validateAlias, type ThawReport } from "@/lib/persona";
import { database, type PersonaRecord } from "@/lib/receipts";
import type { Market } from "@/lib/bettor";
import Page from "@/app/p/[persona_id]/page";

vi.mock("@/lib/motion", () => ({ useReducedMotion: () => true }));
const mocks = vi.hoisted(() => ({ build: vi.fn(), parsed: vi.fn(), persona: vi.fn() }));
vi.mock("@/lib/wasm", () => ({ loadGlassWasm: async () => ({ build_bet: mocks.build, parse_payout_ua: mocks.parsed, new_persona: mocks.persona }) }));
vi.mock("qrcode", () => ({ toDataURL: async () => "data:image/png;base64,AAAA" }));

const ROOT = "5a".repeat(32);
const ID = "eb8c8a530c6b4d4f35410e83febf4f97";
const PASS = "correct horse battery";
const market = { status: "OPEN", rules_hash: "abc", announce_height: 5, market_definition: { market_id: "a".repeat(32), question: "Pool target?", network: "regtest", min_bet_zat: 1000000, close_height: 100, resolve_height: 110, outcomes: [{ outcome_index: 0, label: "YES" }, { outcome_index: 1, label: "NO" }] } } as Market;
const report: ThawReport = { persona_id: ID, alias: "orion", registered_height: 5, stats: { settled: 2, won: 1, win_rate: 0.5, roi: -0.34 }, bets: [
  { market_id: "a".repeat(32), note_ref: "1".repeat(16), outcome_index: 0, amount_zat: 200000000, block_height: 10, result: "WON", payout_zat: 198000000, counted_in_reputation: true, exclusion: null },
  { market_id: "a".repeat(32), note_ref: "2".repeat(16), outcome_index: 1, amount_zat: 100000000, block_height: 11, result: "LOST", payout_zat: null, counted_in_reputation: true, exclusion: null },
  { market_id: "a".repeat(32), note_ref: "3".repeat(16), outcome_index: 0, amount_zat: 100000000, block_height: 2, result: "WON", payout_zat: null, counted_in_reputation: false, exclusion: "PRE_REGISTRATION" },
  { market_id: "a".repeat(32), note_ref: "4".repeat(16), outcome_index: 0, amount_zat: 200000000, block_height: 12, result: "WON", payout_zat: null, counted_in_reputation: false, exclusion: "DUPLICATE_TAG" }] };

async function stored(): Promise<PersonaRecord> {
  const record = { persona_id: ID, frstp1_uri: "zcash:registry?memo=FRSTP1", created_at: 1, enc: await encryptRoot(ROOT, PASS, ID, 1000) };
  await savePersona(record); return record;
}
function api(handlers: Record<string, unknown>) {
  const calls: { url: string; init?: RequestInit }[] = [];
  vi.stubGlobal("fetch", vi.fn(async (url: string, init?: RequestInit) => {
    calls.push({ url, init });
    const hit = Object.entries(handlers).find(([k]) => url.endsWith(k));
    const body = hit ? hit[1] : null;
    const ok = body !== null && !(typeof body === "object" && "status" in (body as object) && (body as { status: number }).status >= 400 && "error" in (body as object));
    return { ok, status: ok ? 200 : (body as { status: number } | null)?.status ?? 404, json: async () => (ok ? body : (body as { body: unknown } | null)?.body ?? null) };
  }));
  return calls;
}
beforeEach(async () => { const db = await database(); await db.clear("personas"); await db.clear("bet_receipts"); db.close(); });
afterEach(() => { cleanup(); vi.unstubAllGlobals(); vi.restoreAllMocks(); vi.clearAllMocks(); });

describe("encrypted Persona storage", () => {
  it("round-trips K_root with AES-GCM and never stores it in plain form", async () => {
    const record = await stored();
    expect(await decryptRoot(record, PASS)).toBe(ROOT);
    expect(JSON.stringify(await getPersona())).not.toContain(ROOT);
    expect(record.enc.kdf).toBe("PBKDF2-SHA256");
  });
  it("rejects a wrong passphrase, a record moved to another id, and weak passphrases", async () => {
    const record = await stored();
    await expect(decryptRoot(record, "not the passphrase")).rejects.toThrow("Wrong passphrase");
    await expect(decryptRoot({ ...record, persona_id: "f".repeat(32) }, PASS)).rejects.toThrow("Wrong passphrase");
    await expect(encryptRoot(ROOT, "short", ID, 1000)).rejects.toThrow("at least");
    await expect(encryptRoot("zz", PASS, ID, 1000)).rejects.toThrow("32 bytes");
  });
  it("encodes K_root as unpadded base64url", () => {
    expect(rootToBase64Url(ROOT)).toBe(Buffer.from(ROOT, "hex").toString("base64url"));
    expect(rootToBase64Url("fb".repeat(32))).not.toMatch(/[+/=]/);
  });
  it("validates alias confirmation and formats reputation", () => {
    expect(validateAlias("orion", "orion")).toBeNull();
    expect(validateAlias("orion", "Orion")).toContain("exactly");
    expect(validateAlias("has space", "has space")).toContain("Alias");
    expect(validateAlias("", "")).toContain("Alias");
    expect(formatRoi(0.38)).toBe("+38%"); expect(formatRoi(-0.34)).toBe("-34%"); expect(formatRoi(null)).toBe("—");
    expect(hasBadge(9)).toBe(false); expect(hasBadge(10)).toBe(true);
  });
});

describe("S-06 Persona", () => {
  it("shows no registration QR until the backup is confirmed, then stores only ciphertext", async () => {
    api({ "/personas/registry": { uivk: "u", address: "uregtest1registry", birthday_height: 1 } });
    mocks.persona.mockReturnValue({ k_root: ROOT, persona_id: ID, frstp1_uri: "zcash:uregtest1registry?memo=FRSTP1" });
    render(<PersonaPanel />);
    fireEvent.click(await screen.findByText("Create Persona"));
    await screen.findByText("01 / Back up your key");
    expect(screen.getByLabelText("Backup key").textContent).toBe(ROOT);
    expect(screen.queryByRole("img")).toBeNull(); expect(screen.queryByLabelText("Registration URI")).toBeNull();
    fireEvent.change(screen.getByLabelText(/^Passphrase/), { target: { value: PASS } });
    fireEvent.change(screen.getByLabelText("Repeat passphrase"), { target: { value: PASS } });
    fireEvent.click(screen.getByText("Encrypt and continue"));
    expect(screen.queryByLabelText("Registration URI")).toBeNull(); // backup box not ticked: fieldset disabled
    expect(await getPersona()).toBeNull();
    fireEvent.click(screen.getByLabelText(/I saved this key/));
    fireEvent.click(screen.getByText("Encrypt and continue"));
    await screen.findByLabelText("Registration URI");
    const record = (await getPersona())!;
    expect(record.persona_id).toBe(ID); expect(JSON.stringify(record)).not.toContain(ROOT);
    expect(await decryptRoot(record, PASS)).toBe(ROOT);
    expect(screen.getByText(/Not registered yet/)).toBeTruthy();
  });
  it("reports the registration height once the indexer records the Persona", async () => {
    await stored();
    api({ [`/personas/${ID}`]: { persona_id: ID, commitment: "c", registered_height: 4480100, status: "REGISTERED", alias: null, thawed_at: null } });
    render(<PersonaPanel />);
    expect((await screen.findByText(/Registered at block 4,480,100/)).textContent).toContain("4,480,100");
    expect(screen.queryByLabelText("Registration URI")).toBeNull();
  });
  it("shows the backup key only after the passphrase decrypts it", async () => {
    await stored(); api({});
    render(<PersonaPanel />);
    fireEvent.change(await screen.findByLabelText("Passphrase"), { target: { value: "wrong wrong wrong" } });
    fireEvent.click(screen.getByText("Show backup key", { selector: "button" }));
    await screen.findByRole("alert"); expect(screen.queryByLabelText("Backup key")).toBeNull();
    fireEvent.change(screen.getByLabelText("Passphrase"), { target: { value: PASS } });
    fireEvent.click(screen.getByText("Show backup key", { selector: "button" }));
    await waitFor(() => expect(screen.getByLabelText("Backup key").textContent).toBe(ROOT));
  });
});

describe("S-07 Thaw", () => {
  const registered = { persona_id: ID, commitment: "c", registered_height: 5, status: "REGISTERED", alias: null, thawed_at: null };
  async function fill(alias: string, confirm: string, pass = PASS) {
    fireEvent.change(await screen.findByLabelText("Public alias"), { target: { value: alias } });
    fireEvent.change(screen.getByLabelText("Type the alias again to confirm"), { target: { value: confirm } });
    fireEvent.change(screen.getByLabelText("Passphrase"), { target: { value: pass } });
  }
  it("warns about permanence, keeps the button disabled until the alias matches, and sends nothing before Thaw", async () => {
    await stored(); const calls = api({ [`/personas/${ID}`]: registered });
    render(<ThawPanel />);
    expect((await screen.findByText(/including losses\. This cannot be undone/)).textContent).toBeTruthy();
    const button = screen.getByText("Thaw permanently") as HTMLButtonElement;
    await waitFor(() => expect(button.disabled).toBe(true));
    await fill("orion", "orio"); expect(button.disabled).toBe(true);
    fireEvent.change(screen.getByLabelText("Type the alias again to confirm"), { target: { value: "orion" } });
    await waitFor(() => expect(button.disabled).toBe(false));
    expect(calls.every((c) => !c.init?.method || c.init.method === "GET")).toBe(true);
    expect(JSON.stringify(calls)).not.toContain(rootToBase64Url(ROOT));
  });
  it("posts K_root and alias on Thaw and shows the report with losses and exclusions", async () => {
    await stored();
    const calls = api({ [`/personas/${ID}/thaw`]: report, [`/personas/${ID}`]: registered, [`/markets/${"a".repeat(32)}`]: market });
    render(<ThawPanel />);
    await fill("orion", "orion");
    await waitFor(() => expect((screen.getByText("Thaw permanently") as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(screen.getByText("Thaw permanently"));
    await screen.findByText("Thawed: orion.");
    const post = calls.find((c) => c.init?.method === "POST")!;
    expect(post.url).toMatch(new RegExp(`/personas/${ID}/thaw$`));
    expect(JSON.parse(post.init!.body as string)).toEqual({ k_root_b64: rootToBase64Url(ROOT), alias: "orion" });
    expect(post.init!.credentials).toBe("omit");
    expect(screen.getByText("Lost")).toBeTruthy();
    expect(screen.getByText("Excluded: Before registration")).toBeTruthy();
    expect(screen.getByText("Excluded: Duplicate tag")).toBeTruthy();
  });
  it("maps a commitment mismatch to a clear message and publishes nothing", async () => {
    await stored();
    api({ [`/personas/${ID}/thaw`]: { status: 400, error: true, body: { error: { code: "THAW_COMMITMENT_MISMATCH", message: "x" } } }, [`/personas/${ID}`]: registered });
    render(<ThawPanel />);
    await fill("orion", "orion");
    await waitFor(() => expect((screen.getByText("Thaw permanently") as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(screen.getByText("Thaw permanently"));
    expect((await screen.findByRole("alert")).textContent).toContain("does not match the registered Persona");
    expect(screen.queryByText(/^Thawed:/)).toBeNull();
  });
  it("sends nothing when the passphrase is wrong", async () => {
    await stored(); const calls = api({ [`/personas/${ID}`]: registered });
    render(<ThawPanel />);
    await fill("orion", "orion", "wrong wrong wrong");
    await waitFor(() => expect((screen.getByText("Thaw permanently") as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(screen.getByText("Thaw permanently"));
    expect((await screen.findByRole("alert")).textContent).toContain("Wrong passphrase");
    expect(calls.some((c) => c.init?.method === "POST")).toBe(false);
  });
});

describe("Persona toggle in the Bet Builder", () => {
  async function toSeal(passphrase?: string) {
    fireEvent.click(screen.getByText("Continue")); fireEvent.click(screen.getByText("Continue"));
    fireEvent.change(screen.getByLabelText(/Payout Unified Address/), { target: { value: "uregtest1private" } });
    if (passphrase !== undefined) { fireEvent.click(await screen.findByLabelText("Tag this bet with my Persona")); fireEvent.change(screen.getByLabelText(/Persona passphrase/), { target: { value: passphrase } }); }
    fireEvent.click(screen.getByText("Continue"));
  }
  it("passes the decrypted K_root to build_bet and records the receipt as tagged", async () => {
    await stored(); api({});
    mocks.parsed.mockReturnValue({ ok: true });
    mocks.build.mockReturnValue({ memo_text: "FRST1:x", zip321_uri: "zcash:x", bet_fingerprint: "6".repeat(16), bet_salt: "7".repeat(16), env_esk: "8".repeat(64) });
    render(<BetBuilder market={market} />);
    await toSeal(PASS); await screen.findByText("04 / Review & seal");
    expect(screen.getByText(/Persona tagged/)).toBeTruthy();
    fireEvent.click(screen.getByText("Seal & save receipt")); await screen.findByText("Receipt saved on this device.");
    expect(mocks.build.mock.calls[0][0].k_root).toBe(ROOT);
  });
  it("refuses a wrong passphrase before sealing", async () => {
    await stored(); api({}); mocks.parsed.mockReturnValue({ ok: true });
    render(<BetBuilder market={market} />);
    await toSeal("wrong wrong wrong");
    expect((await screen.findByRole("alert")).textContent).toContain("Wrong passphrase");
    expect(screen.queryByText("04 / Review & seal")).toBeNull(); expect(mocks.build).not.toHaveBeenCalled();
  });
  it("omits the tag when the toggle is off and offers Persona creation when none exists", async () => {
    api({}); mocks.parsed.mockReturnValue({ ok: true });
    mocks.build.mockReturnValue({ memo_text: "FRST1:x", zip321_uri: "zcash:x", bet_fingerprint: "9".repeat(16), bet_salt: "7".repeat(16), env_esk: "8".repeat(64) });
    render(<BetBuilder market={market} />);
    await toSeal(); await screen.findByText("04 / Review & seal");
    fireEvent.click(screen.getByText("Seal & save receipt")); await screen.findByText("Receipt saved on this device.");
    expect(mocks.build.mock.calls[0][0]).not.toHaveProperty("k_root");
  });
});

describe("S-08 and S-09", () => {
  it("lists every bet including losses and exclusion reasons", () => {
    render(<ThawReportView report={report} markets={{ ["a".repeat(32)]: { question: "Pool target?", outcomes: market.market_definition.outcomes } }} />);
    expect(screen.getAllByRole("row")).toHaveLength(5);
    expect(screen.getByText("Lost")).toBeTruthy(); expect(screen.getAllByText("Pool target?")).toHaveLength(4);
    expect(screen.getByText("-34%")).toBeTruthy(); expect(screen.getByText("50%")).toBeTruthy();
  });
  it("renders the profile of a thawed Persona and withholds an unthawed one", async () => {
    api({ [`/personas/${ID}`]: { persona_id: ID, commitment: "c", registered_height: 5, status: "THAWED", alias: "orion", thawed_at: 1 }, [`/personas/${ID}/report`]: report, [`/markets/${"a".repeat(32)}`]: market });
    render(await Page({ params: Promise.resolve({ persona_id: ID }) }));
    expect(screen.getByRole("heading", { name: "orion." })).toBeTruthy(); expect(screen.getByText("Lost")).toBeTruthy();
    cleanup();
    api({ [`/personas/${ID}`]: { persona_id: ID, commitment: "c", registered_height: 5, status: "REGISTERED", alias: null, thawed_at: null } });
    render(await Page({ params: Promise.resolve({ persona_id: ID }) }));
    expect(screen.getByText(/has not thawed/)).toBeTruthy(); expect(screen.queryByRole("table")).toBeNull();
  });
  it("shows the 10+ badge from 10 settled bets and the Sybil note", () => {
    const entry = { persona_id: ID, alias: "orion", registered_height: 4480100, settled: 14, won: 10, win_rate: 0.714, roi: 0.38 };
    render(<LeaderboardTable entries={[entry, { ...entry, persona_id: "b".repeat(32), alias: "kestrel", settled: 6, won: 4, win_rate: 0.667, roi: 0.52 }]} />);
    expect(screen.getAllByText("10+")).toHaveLength(1);
    expect(screen.getByText("71%")).toBeTruthy(); expect(screen.getByText("+38%")).toBeTruthy();
    expect(screen.getByText(/One person may control several Personas/)).toBeTruthy();
    cleanup(); render(<LeaderboardTable entries={[]} />); expect(screen.getByText("No thawed Personas yet.")).toBeTruthy();
    cleanup(); render(<LeaderboardTable entries={null} />); expect(screen.getByRole("status").textContent).toContain("—");
  });
});
