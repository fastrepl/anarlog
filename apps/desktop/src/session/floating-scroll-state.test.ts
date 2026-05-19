import { describe, expect, it } from "vitest";

import { getNextFloatingButtonHidden } from "./floating-scroll-state";

describe("getNextFloatingButtonHidden", () => {
  it("hides when scrolling down", () => {
    expect(
      getNextFloatingButtonHidden({
        currentHidden: false,
        delta: 12,
        scrollTop: 120,
        scrollHeight: 1000,
        clientHeight: 400,
      }),
    ).toBe(true);
  });

  it("shows when scrolling up away from the bottom", () => {
    expect(
      getNextFloatingButtonHidden({
        currentHidden: true,
        delta: -12,
        scrollTop: 420,
        scrollHeight: 1000,
        clientHeight: 400,
      }),
    ).toBe(false);
  });

  it("stays hidden during bottom bounce", () => {
    expect(
      getNextFloatingButtonHidden({
        currentHidden: true,
        delta: -12,
        scrollTop: 552,
        scrollHeight: 1000,
        clientHeight: 400,
      }),
    ).toBe(true);
  });

  it("shows near the top", () => {
    expect(
      getNextFloatingButtonHidden({
        currentHidden: true,
        delta: 0,
        scrollTop: 4,
        scrollHeight: 1000,
        clientHeight: 400,
      }),
    ).toBe(false);
  });
});
