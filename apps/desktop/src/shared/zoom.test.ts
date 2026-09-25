import { describe, expect, it, vi } from "vitest";

import {
  DEFAULT_ZOOM_FACTOR,
  persistZoomFactor,
  readZoomFactor,
  stepZoomFactor,
  ZOOM_STORAGE_KEY,
  ZOOM_STEPS,
} from "./zoom";

describe("stepZoomFactor", () => {
  it("steps in from the default", () => {
    expect(stepZoomFactor(1, "in")).toBe(1.1);
  });

  it("steps out from the default", () => {
    expect(stepZoomFactor(1, "out")).toBe(0.9);
  });

  it("clamps at the bounds", () => {
    const max = ZOOM_STEPS[ZOOM_STEPS.length - 1];
    const min = ZOOM_STEPS[0];
    expect(stepZoomFactor(max, "in")).toBe(max);
    expect(stepZoomFactor(min, "out")).toBe(min);
  });

  it("returns the default on reset", () => {
    expect(stepZoomFactor(2, "reset")).toBe(DEFAULT_ZOOM_FACTOR);
  });

  it("moves to the next step when between steps", () => {
    expect(stepZoomFactor(1.05, "in")).toBe(1.1);
    expect(stepZoomFactor(1.05, "out")).toBe(1);
  });
});

describe("readZoomFactor", () => {
  const storage = (value: string | null) => ({ getItem: vi.fn(() => value) });

  it("defaults to 1 without a stored value", () => {
    expect(readZoomFactor(storage(null))).toBe(1);
  });

  it("reads a stored factor", () => {
    expect(readZoomFactor(storage("1.25"))).toBe(1.25);
  });

  it("falls back on garbage", () => {
    expect(readZoomFactor(storage("bogus"))).toBe(1);
    expect(readZoomFactor(storage("-2"))).toBe(1);
  });
});

describe("persistZoomFactor", () => {
  it("writes the factor to storage", () => {
    const setItem = vi.fn();
    persistZoomFactor(1.5, { setItem });
    expect(setItem).toHaveBeenCalledWith(ZOOM_STORAGE_KEY, "1.5");
  });
});
