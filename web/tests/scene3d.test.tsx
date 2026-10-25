import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { PageHead } from "@/components/ui/Scene3D";

describe("PageHead", () => {
  it("renders the heading and copy; the decorative scene is hidden from assistive tech and needs no WebGL", () => {
    const { container } = render(<PageHead index="S-01 / TEST" title="The ledger is open." lede="Read the numbers." kind="towers" yes={0.6} no={0.4} />);
    expect(screen.getByRole("heading", { level: 1, name: "The ledger is open." })).toBeTruthy();
    expect(screen.getByText("Read the numbers.")).toBeTruthy();
    expect(container.querySelector(".scene3d")?.getAttribute("aria-hidden")).toBe("true");
    expect(container.querySelector("canvas")).toBeNull();
  });
});
