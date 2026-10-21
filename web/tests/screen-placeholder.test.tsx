import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ScreenPlaceholder } from "@/components/ScreenPlaceholder";

describe("ScreenPlaceholder", () => {
  it("renders the screen id and title", () => {
    render(<ScreenPlaceholder id="S-01" title="Markets" features="F-04" />);
    expect(screen.getByRole("heading", { name: "Markets" })).toBeTruthy();
    expect(screen.getByText("S-01 · F-04")).toBeTruthy();
  });
});
