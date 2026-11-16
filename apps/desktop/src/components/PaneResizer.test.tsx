import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";

import { clampWidth, LIST_WIDTH, PaneResizer, useListWidth } from "./PaneResizer";

function Harness() {
  const [width, setWidth] = useListWidth();
  return <PaneResizer width={width} onChange={setWidth} />;
}

describe("PaneResizer", () => {
  beforeEach(() => localStorage.clear());

  it("drags the list wider within limits and remembers it", () => {
    render(<Harness />);
    const line = screen.getByRole("separator", { name: "Resize the item list" });
    expect(line).toHaveAttribute("aria-valuenow", "300");
    fireEvent.pointerDown(line, { clientX: 500, pointerId: 1 });
    fireEvent.pointerMove(line, { clientX: 620, pointerId: 1 });
    fireEvent.pointerUp(line, { pointerId: 1 });
    expect(line).toHaveAttribute("aria-valuenow", "420");
    expect(localStorage.getItem("password365.listWidth")).toBe("420");
    fireEvent.pointerMove(line, { clientX: 2000 });
    expect(line, "moves only while dragging").toHaveAttribute("aria-valuenow", "420");
  });

  it("works with the keyboard and resets on double-click", () => {
    render(<Harness />);
    const line = screen.getByRole("separator");
    fireEvent.keyDown(line, { key: "ArrowRight" });
    expect(line).toHaveAttribute("aria-valuenow", "316");
    fireEvent.keyDown(line, { key: "ArrowLeft" });
    fireEvent.keyDown(line, { key: "ArrowLeft" });
    expect(line).toHaveAttribute("aria-valuenow", "284");
    fireEvent.keyDown(line, { key: "Enter" });
    fireEvent.doubleClick(line);
    expect(line).toHaveAttribute("aria-valuenow", String(LIST_WIDTH.initial));
  });

  it("starts from the remembered width, kept within limits", () => {
    localStorage.setItem("password365.listWidth", "9999");
    render(<Harness />);
    expect(screen.getByRole("separator")).toHaveAttribute("aria-valuenow", String(LIST_WIDTH.max));
    expect(clampWidth(10)).toBe(LIST_WIDTH.min);
  });

  it("copes without storage", () => {
    const get = vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    const set = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    function Plain() {
      const [w, setW] = useState(300);
      return <PaneResizer width={w} onChange={setW} />;
    }
    render(<><Harness /><Plain /></>);
    fireEvent.keyDown(screen.getAllByRole("separator")[0]!, { key: "ArrowRight" });
    expect(screen.getAllByRole("separator")[0]).toHaveAttribute("aria-valuenow", "316");
    get.mockRestore();
    set.mockRestore();
  });
});
