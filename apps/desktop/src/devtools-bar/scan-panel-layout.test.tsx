import { afterEach, beforeEach, expect, it, vi } from "vitest";

import { constrainPanel, resizePanel } from "./scan-panel-layout";

beforeEach(() => {
  vi.stubGlobal("innerWidth", 1200);
  vi.stubGlobal("innerHeight", 900);
});
afterEach(() => {
  vi.unstubAllGlobals();
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
