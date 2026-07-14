import { cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  settings: {
    current: {
      floating_bar_opacity: 0.78,
      live_caption_opacity: 0.3,
      live_caption_width: 440,
      live_caption_line_count: 1,
      live_caption_position: "topCenter",
      live_caption_minimized: true,
    },
  },
  floatingBarShow: vi.fn(async () => ({ status: "ok", data: null })),
  floatingBarHide: vi.fn(async () => ({ status: "ok", data: null })),
  floatingBarUpdate: vi.fn(async () => ({ status: "ok", data: null })),
  liveCaptionHide: vi.fn(async () => ({ status: "ok", data: null })),
  windowShow: vi.fn(async () => ({ status: "ok", data: null })),
  listen: vi.fn(async () => vi.fn()),
  setSettingValue: vi.fn(async () => undefined),
  setSettingValues: vi.fn(),
  subscribeMeetingFloatData: vi.fn(async () => vi.fn(async () => undefined)),
  listenerState: {
    live: {
      status: "active",
      sessionId: "session-1",
      amplitude: { mic: 0, speaker: 0 },
      degraded: null,
      lastError: null,
      liveTranscriptionActive: true,
    },
    liveSegments: [],
    stop: vi.fn(),
  },
  subscribeListener: vi.fn(() => vi.fn()),
}));

vi.mock("@hypr/plugin-windows", () => ({
  commands: {
    floatingBarShow: mocks.floatingBarShow,
    floatingBarHide: mocks.floatingBarHide,
    floatingBarUpdate: mocks.floatingBarUpdate,
    liveCaptionHide: mocks.liveCaptionHide,
    windowShow: mocks.windowShow,
  },
  events: {
    floatingBarOpenMain: { listen: mocks.listen },
    floatingBarSettingsChange: { listen: mocks.listen },
    floatingBarStop: { listen: mocks.listen },
  },
}));

vi.mock("./hooks", () => ({
  createMeetingFloatLabelContext: vi.fn(() => undefined),
  loadMeetingFloatData: vi.fn(async () => ({ sessions: {}, humanNames: {} })),
  subscribeMeetingFloatData: mocks.subscribeMeetingFloatData,
}));

vi.mock("~/settings/queries", () => ({
  getStoredSettingValues: vi.fn(async () => ({
    values: mocks.settings.current,
    hasValues: new Set(Object.keys(mocks.settings.current)),
  })),
  setSettingValue: mocks.setSettingValue,
  useSetSettingValues: () => mocks.setSettingValues,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: () => true,
  useConfigValues: () => mocks.settings.current,
}));

vi.mock("~/store/zustand/listener/instance", () => ({
  listenerStore: {
    getState: () => mocks.listenerState,
    subscribe: mocks.subscribeListener,
  },
}));

import { FloatingMeetingWindowHost } from "./host";

describe("FloatingMeetingWindowHost", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.settings.current = {
      floating_bar_opacity: 0.78,
      live_caption_opacity: 0.3,
      live_caption_width: 440,
      live_caption_line_count: 1,
      live_caption_position: "topCenter",
      live_caption_minimized: true,
    };
  });

  afterEach(() => {
    cleanup();
  });

  it("updates overlay settings without hiding the active floating panel", async () => {
    const view = render(<FloatingMeetingWindowHost />);

    await waitFor(() => {
      expect(mocks.floatingBarShow).toHaveBeenCalledOnce();
      expect(mocks.floatingBarUpdate).toHaveBeenLastCalledWith(
        expect.objectContaining({ liveCaptionMinimized: true }),
      );
    });

    mocks.settings.current = {
      ...mocks.settings.current,
      live_caption_minimized: false,
    };
    view.rerender(<FloatingMeetingWindowHost />);

    await waitFor(() => {
      expect(mocks.floatingBarUpdate).toHaveBeenLastCalledWith(
        expect.objectContaining({ liveCaptionMinimized: false }),
      );
    });
    expect(mocks.floatingBarShow).toHaveBeenCalledOnce();
    expect(mocks.floatingBarHide).not.toHaveBeenCalled();
  });
});
