import { act, cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

import {
  constrainPanel,
  resizePanel,
  useScanPanelLayout,
} from "./scan-panel-layout";

function Panel() {
  const layout = useScanPanelLayout();
  return (
    <section
      ref={layout.panelRef}
      style={layout.style}
      {...layout.handlers}
      data-testid="panel"
    >
      <header onPointerDown={layout.drag}>
        Drag <button type="button">Action</button>
      </header>
      <div
        data-testid="resize"
        onPointerDown={(event) => layout.resize(event, "nw")}
      />
    </section>
  );
}
beforeEach(() => {
  localStorage.clear();
  vi.stubGlobal("innerWidth", 1200);
  vi.stubGlobal("innerHeight", 900);
  vi.stubGlobal(
    "PointerEvent",
    class extends MouseEvent {
      pointerId: number;
      constructor(type: string, init: PointerEventInit) {
        super(type, init);
        this.pointerId = init.pointerId ?? 1;
      }
    },
  );
  HTMLElement.prototype.setPointerCapture = vi.fn();
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  delete (HTMLElement.prototype as Partial<HTMLElement>).setPointerCapture;
});

it("keeps offscreen saved geometry inside a smaller window", () => {
  expect(
    constrainPanel(
      { x: 1900, y: 1200, width: 1000, height: 800 },
      { width: 800, height: 600 },
    ),
  ).toEqual({ x: 8, y: 32, width: 784, height: 538 });
});
it("resizes from the top left while anchoring the opposite corner and enforcing minimum size", () => {
  const rect = { x: 200, y: 150, width: 800, height: 600 };
  expect(resizePanel(rect, "nw", 100, 100)).toEqual({
    x: 300,
    y: 250,
    width: 700,
    height: 500,
  });
  expect(resizePanel(rect, "nw", 1000, 1000)).toEqual({
    x: 440,
    y: 490,
    width: 560,
    height: 260,
  });
});
it("drags, resizes, restores the saved rectangle, and stays visible after window resize", () => {
  const screen = render(<Panel />);
  const panel = screen.getByTestId("panel");
  const header = screen.getByText("Drag");
  fireEvent.pointerDown(header, {
    button: 0,
    pointerId: 1,
    clientX: 500,
    clientY: 500,
  });
  fireEvent.pointerMove(header, { pointerId: 1, clientX: 400, clientY: 300 });
  fireEvent.pointerUp(header, { pointerId: 1 });
  expect(panel.style.left).toBe("192px");
  expect(panel.style.top).toBe("210px");
  const handle = screen.getByTestId("resize");
  fireEvent.pointerDown(handle, {
    button: 0,
    pointerId: 2,
    clientX: 192,
    clientY: 210,
  });
  fireEvent.pointerMove(handle, { pointerId: 2, clientX: 292, clientY: 310 });
  fireEvent.pointerUp(handle, { pointerId: 2 });
  expect(panel.style.width).toBe("800px");
  expect(panel.style.height).toBe("360px");
  screen.unmount();
  const restored = render(<Panel />).getByTestId("panel");
  expect(restored.style.left).toBe("292px");
  expect(restored.style.width).toBe("800px");
  vi.stubGlobal("innerWidth", 700);
  act(() => {
    window.dispatchEvent(new Event("resize"));
  });
  expect(restored.style.left).toBe("8px");
  expect(restored.style.width).toBe("684px");
});
it("does not drag when a header button is pressed and ignores malformed preferences", () => {
  localStorage.setItem("anarlog:devtools-scan-layout", '{"x":"bad"}');
  const screen = render(<Panel />);
  const panel = screen.getByTestId("panel");
  const before = panel.style.cssText;
  const button = screen.getByRole("button", { name: "Action" });
  fireEvent.pointerDown(button, { button: 0, clientX: 500, clientY: 500 });
  fireEvent.pointerMove(button, { clientX: 200, clientY: 200 });
  fireEvent.pointerUp(button);
  expect(panel.style.cssText).toBe(before);
});
