import { render, cleanup } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { BlockTicker } from "@/components/ui/BlockTicker";

const mocks = vi.hoisted(() => ({ pathname: "/wasm-test", fetchTip: vi.fn(async () => 10) }));
vi.mock("next/navigation", () => ({ usePathname: () => mocks.pathname }));
vi.mock("@/lib/api", () => ({ fetchTip: mocks.fetchTip }));
vi.mock("@/components/ui/Numbers", () => ({ RollingNumber: () => <span>—</span> }));
afterEach(() => { cleanup(); mocks.fetchTip.mockClear(); });

it("keeps the WASM harness offline and resumes API polling on normal pages", () => {
  const page = render(<BlockTicker />);
  expect(mocks.fetchTip).not.toHaveBeenCalled();
  mocks.pathname = "/how";
  page.rerender(<BlockTicker />);
  expect(mocks.fetchTip).toHaveBeenCalledOnce();
});
