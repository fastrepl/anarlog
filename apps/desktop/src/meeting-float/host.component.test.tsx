import { act, cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  enabled: true,
  dictationAction: null as
    | null
    | ((event: { payload: { sessionId: string; action: string } }) => void),
  meetingStop: null as null | (() => void),
  platform: vi.fn(() => "macos"),
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
  liveCaptionShow: vi.fn(async () => ({ status: "ok", data: null })),
  liveCaptionHide: vi.fn(async () => ({ status: "ok", data: null })),
  liveCaptionUpdate: vi.fn(async () => ({ status: "ok", data: null })),
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
    liveCaptionText: "we should ship this",
    stop: vi.fn(),
  },
  subscribeListener: vi.fn(() => vi.fn()),
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: mocks.platform,
}));

vi.mock("@anlg/plugin-windows", () => ({
  commands: {
    floatingBarShow: mocks.floatingBarShow,
    floatingBarHide: mocks.floatingBarHide,
    floatingBarUpdate: mocks.floatingBarUpdate,
    liveCaptionShow: mocks.liveCaptionShow,
    liveCaptionHide: mocks.liveCaptionHide,
    liveCaptionUpdate: mocks.liveCaptionUpdate,
    windowShow: mocks.windowShow,
  },
  events: {
    floatingBarOpenMain: { listen: mocks.listen },
    floatingBarSettingsChange: { listen: mocks.listen },
    floatingBarStop: {
      listen: vi.fn(async (callback) => {
        mocks.meetingStop = callback;
        return vi.fn();
      }),
    },
    floatingBarDictationAction: {
      listen: vi.fn(async (callback) => {
        mocks.dictationAction = callback;
        return vi.fn();
      }),
    },
  },
}));

vi.mock("./hooks", () => ({
  createMeetingFloatLabelContext: vi.fn(() => undefined),
  createMeetingFloatRenderRequest: vi.fn(() => null),
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
  useConfigValue: () => mocks.enabled,
  useConfigValues: () => mocks.settings.current,
}));

vi.mock("~/store/zustand/listener/instance", () => ({
  listenerStore: {
    getState: () => mocks.listenerState,
    subscribe: mocks.subscribeListener,
  },
}));

import { FloatingMeetingWindowHost } from "./host";

import { useDictationStatus } from "~/dictation/state";

describe("FloatingMeetingWindowHost", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.meetingStop = null;
    mocks.dictationAction = null;
    mocks.enabled = true;
    mocks.listenerState.live.status = "active";
    useDictationStatus.setState(useDictationStatus.getInitialState());
    mocks.platform.mockReturnValue("macos");
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
        expect.objectContaining({
          liveCaptionMinimized: false,
          transcriptBubbles: null,
        }),
      );
    });
    expect(mocks.liveCaptionShow).not.toHaveBeenCalled();
    expect(mocks.liveCaptionHide).toHaveBeenCalled();
    expect(mocks.floatingBarShow).toHaveBeenCalledOnce();
    expect(mocks.floatingBarHide).not.toHaveBeenCalled();
  });

  it.each(["windows", "linux"] as const)(
    "uses the floating bar on %s like macOS, without the old caption overlay",
    async (currentPlatform) => {
      mocks.platform.mockReturnValue(currentPlatform);

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
      expect(mocks.liveCaptionHide).toHaveBeenCalled();
      expect(mocks.liveCaptionShow).not.toHaveBeenCalled();
      expect(mocks.liveCaptionUpdate).not.toHaveBeenCalled();
      expect(mocks.floatingBarHide).not.toHaveBeenCalled();
    },
  );
  it.each(["macos", "windows", "linux"])(
    "keeps dictation usable when meeting panels are disabled on %s",
    async (platform) => {
      mocks.platform.mockReturnValue(platform);
      mocks.enabled = false;
      mocks.listenerState.live.status = "inactive";
      const finish = vi.fn();
      const cancel = vi.fn();
      useDictationStatus.setState({
        phase: "recording",
        owner: "dictation-1",
        text: "Hello",
        finish,
        cancel,
      });
      render(<FloatingMeetingWindowHost />);
      await waitFor(() =>
        expect(mocks.floatingBarUpdate).toHaveBeenLastCalledWith(
          expect.objectContaining({
            dictation: expect.objectContaining({ text: "Hello" }),
          }),
        ),
      );
      expect(mocks.floatingBarUpdate.mock.invocationCallOrder[0]).toBeLessThan(
        mocks.floatingBarShow.mock.invocationCallOrder[0]!,
      );
      await act(async () => {
        mocks.dictationAction!({
          payload: { sessionId: "old-recording", action: "finish" },
        });
        mocks.meetingStop!();
      });
      expect(finish).not.toHaveBeenCalled();
      expect(mocks.listenerState.stop).not.toHaveBeenCalled();
      expect(mocks.floatingBarHide).not.toHaveBeenCalled();
      await act(async () => {
        mocks.dictationAction!({
          payload: { sessionId: "dictation-1", action: "togglePreview" },
        });
      });
      await waitFor(() =>
        expect(mocks.floatingBarUpdate).toHaveBeenLastCalledWith(
          expect.objectContaining({ liveCaptionMinimized: false }),
        ),
      );
      await act(async () => {
        mocks.dictationAction!({
          payload: { sessionId: "dictation-1", action: "finish" },
        });
        mocks.dictationAction!({
          payload: { sessionId: "dictation-1", action: "cancel" },
        });
      });
      expect(finish).toHaveBeenCalledOnce();
      expect(cancel).toHaveBeenCalledOnce();
      await act(async () => {
        useDictationStatus.setState({ phase: "idle" });
      });
      await waitFor(() => expect(mocks.floatingBarHide).toHaveBeenCalledOnce());
    },
  );

  it("gives an active meeting ownership of the panel", async () => {
    useDictationStatus.setState({ phase: "recording", owner: "dictation-1" });
    render(<FloatingMeetingWindowHost />);
    await waitFor(() =>
      expect(mocks.floatingBarUpdate).toHaveBeenLastCalledWith(
        expect.objectContaining({ dictation: null }),
      ),
    );
    expect(mocks.floatingBarShow).toHaveBeenCalledOnce();
  });
});
