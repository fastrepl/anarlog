import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  sidebarTimelineEnabled: false,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: () => mocks.sidebarTimelineEnabled,
}));

import { useLeftSidebar } from "./leftsidebar";

describe("useLeftSidebar", () => {
  beforeEach(() => {
    mocks.sidebarTimelineEnabled = false;
  });

  it("toggles expansion outside sidebar timeline mode", () => {
    const { result } = renderHook(() => useLeftSidebar());

    expect(result.current.expanded).toBe(true);

    act(() => result.current.toggleExpanded());

    expect(result.current.expanded).toBe(false);

    act(() => result.current.setExpanded(true));

    expect(result.current.expanded).toBe(true);
  });

  it("keeps the sidebar open and locked in sidebar timeline mode", () => {
    const { result, rerender } = renderHook(() => useLeftSidebar());

    act(() => result.current.setExpanded(false));

    expect(result.current.expanded).toBe(false);

    mocks.sidebarTimelineEnabled = true;
    rerender();

    expect(result.current.expanded).toBe(true);
    expect(result.current.locked).toBe(true);

    act(() => result.current.setExpanded(false));

    expect(result.current.expanded).toBe(true);

    act(() => result.current.toggleExpanded());

    expect(result.current.expanded).toBe(true);
  });

  it("preserves collapsed preference while sidebar timeline mode forces expansion", () => {
    const { result, rerender } = renderHook(() => useLeftSidebar());

    act(() => result.current.setExpanded(false));

    expect(result.current.expanded).toBe(false);

    mocks.sidebarTimelineEnabled = true;
    rerender();

    const forcedExpanded = result.current.expanded;
    expect(forcedExpanded).toBe(true);

    act(() => result.current.setExpanded(forcedExpanded));

    expect(result.current.expanded).toBe(true);

    mocks.sidebarTimelineEnabled = false;
    rerender();

    expect(result.current.expanded).toBe(false);
  });
});
