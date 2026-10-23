import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Select } from "@/components/ui/Select";

const OPTIONS = [{ value: "", label: "All" }, { value: "open", label: "Open" }, { value: "closed", label: "Closed" }, { value: "settled", label: "Settled" }];
function Harness({ onChange = () => {}, initial = "" }: { onChange?: (v: string) => void; initial?: string }) {
  const [v, setV] = useState(initial);
  return <><Select id="s" label="Status" value={v} options={OPTIONS} onChange={(x) => { setV(x); onChange(x); }} /><button>outside</button><p data-testid="v">{v}</p></>;
}
const box = () => screen.getByRole("combobox");
afterEach(cleanup);

describe("custom Select", () => {
  it("is a labelled combobox that opens a listbox and never renders a native select", () => {
    render(<Harness />);
    expect(document.querySelector("select")).toBeNull();
    expect(screen.getByLabelText("Status")).toBe(box());
    expect(box().getAttribute("aria-expanded")).toBe("false");
    expect(screen.queryByRole("listbox")).toBeNull();
    fireEvent.click(box());
    expect(box().getAttribute("aria-expanded")).toBe("true");
    expect(screen.getAllByRole("option")).toHaveLength(4);
    expect(screen.getByRole("option", { name: "All" }).getAttribute("aria-selected")).toBe("true");
    expect(box().getAttribute("aria-controls")).toBe(screen.getByRole("listbox").id);
  });

  it("selects by click and closes", () => {
    const onChange = vi.fn();
    render(<Harness onChange={onChange} />);
    fireEvent.click(box()); fireEvent.click(screen.getByRole("option", { name: "Closed" }));
    expect(onChange).toHaveBeenCalledWith("closed");
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(box().textContent).toContain("Closed");
  });

  it("works from the keyboard: arrows, Home/End, Enter, Space, Escape", () => {
    render(<Harness />);
    fireEvent.keyDown(box(), { key: "ArrowDown" });                       // opens on the selected option
    expect(screen.getByRole("listbox")).toBeTruthy();
    fireEvent.keyDown(box(), { key: "ArrowDown" }); fireEvent.keyDown(box(), { key: "ArrowDown" });
    expect(box().getAttribute("aria-activedescendant")).toBe("s-opt-2");
    fireEvent.keyDown(box(), { key: "Enter" });
    expect(screen.getByTestId("v").textContent).toBe("closed");
    fireEvent.keyDown(box(), { key: " " });                                // Space opens, on the selected one
    expect(box().getAttribute("aria-activedescendant")).toBe("s-opt-2");
    fireEvent.keyDown(box(), { key: "End" }); expect(box().getAttribute("aria-activedescendant")).toBe("s-opt-3");
    fireEvent.keyDown(box(), { key: "ArrowDown" }); expect(box().getAttribute("aria-activedescendant")).toBe("s-opt-3");   // clamped
    fireEvent.keyDown(box(), { key: "Home" }); expect(box().getAttribute("aria-activedescendant")).toBe("s-opt-0");
    fireEvent.keyDown(box(), { key: "Escape" });
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(screen.getByTestId("v").textContent).toBe("closed");            // Escape changes nothing
    fireEvent.keyDown(box(), { key: "ArrowUp" });                          // closed: opens on the selected option ("Closed")
    fireEvent.keyDown(box(), { key: "ArrowUp" }); fireEvent.keyDown(box(), { key: "ArrowUp" });
    fireEvent.keyDown(box(), { key: "Enter" });
    expect(screen.getByTestId("v").textContent).toBe("");
  });

  it("closes on an outside press and on Tab without changing the value", () => {
    render(<Harness initial="open" />);
    fireEvent.click(box()); expect(screen.getByRole("listbox")).toBeTruthy();
    fireEvent.pointerDown(screen.getByText("outside")); expect(screen.queryByRole("listbox")).toBeNull();
    fireEvent.click(box()); fireEvent.keyDown(box(), { key: "Tab" }); expect(screen.queryByRole("listbox")).toBeNull();
    expect(screen.getByTestId("v").textContent).toBe("open");
  });

  it("jumps to a label by typing its first letters", () => {
    render(<Harness />);
    fireEvent.keyDown(box(), { key: "c" });
    expect(box().getAttribute("aria-activedescendant")).toBe("s-opt-2");
    fireEvent.keyDown(box(), { key: "Enter" });
    expect(screen.getByTestId("v").textContent).toBe("closed");
  });

  it("keeps focus on the button when an option is pressed, so the keyboard keeps working", () => {
    render(<Harness />);
    box().focus(); fireEvent.click(box());
    const notPrevented = fireEvent.pointerDown(screen.getByRole("option", { name: "Open" }));
    expect(notPrevented).toBe(false);   // preventDefault stops the button losing focus
  });
});
