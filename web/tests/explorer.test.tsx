import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const txid = "ab".repeat(32);

afterEach(() => { vi.unstubAllEnvs(); vi.resetModules(); });

describe("explorer links", () => {
  it("shows the id without a link on a network with no explorer", async () => {
    vi.stubEnv("NEXT_PUBLIC_EXPLORER_URL", "");
    const { TxRef, BlockRef } = await import("@/components/ui/ChainRef");
    render(<><TxRef txid={txid} /><BlockRef height={4532238} /></>);
    expect(screen.getByText(txid)).toBeTruthy();
    expect(screen.queryByRole("link")).toBeNull();
    expect(screen.getByText("4,532,238")).toBeTruthy();
  });

  it("links the transaction and block to the configured explorer", async () => {
    vi.stubEnv("NEXT_PUBLIC_EXPLORER_URL", "https://testnet.zecblock.com/");
    const { TxRef, BlockRef } = await import("@/components/ui/ChainRef");
    render(<><TxRef txid={txid} /><BlockRef height={4532238} /></>);
    expect(screen.getByRole("link", { name: "View on explorer" }).getAttribute("href")).toBe(`https://testnet.zecblock.com/tx/${txid}`);
    expect(screen.getByRole("link", { name: /Block 4,532,238/ }).getAttribute("href")).toBe("https://testnet.zecblock.com/block/4532238");
  });
});
