import { act, render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  session: { user: { id: "user-1" } } as { user: { id: string } } | null,
  billing: { isPro: true, isReady: true },
  platform: "macos",
  settings: {
    dictation_enabled: true,
    dictation_shortcut: "Control+Alt+Space",
    dictation_hands_free: false,
    microphone_device: "",
  } as Record<string, unknown>,
  meeting: { status: "inactive", loading: false },
  configure: vi.fn(),
  setActive: vi.fn(),
  show: vi.fn(),
  hide: vi.fn(),
  setPhase: vi.fn(),
  captureTarget: vi.fn(),
  startRecording: vi.fn(),
  stopRecording: vi.fn(),
  cancelRecording: vi.fn(),
  discardRecording: vi.fn(),
  insertText: vi.fn(),
  checkPermission: vi.fn(),
  getCaptureState: vi.fn(),
  runBatch: vi.fn(),
  stopTranscription: vi.fn(),
  unlisten: vi.fn(),
  listener: null as ((event: { payload: { type: string } }) => void) | null,
}));
vi.mock("~/auth", () => ({ useAuth: () => ({ session: mocks.session }) }));
vi.mock("@tauri-apps/plugin-os", () => ({ platform: () => mocks.platform }));
vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => mocks.billing,
}));
vi.mock("~/settings/queries", () => ({ useSettingsReady: () => true }));
vi.mock("~/shared/config", () => ({
  useConfigValue: (key: string) => mocks.settings[key],
}));
vi.mock("~/stt/contexts", () => ({
  useListener: (select: (state: unknown) => unknown) =>
    select({ live: mocks.meeting, stopTranscription: mocks.stopTranscription }),
}));
vi.mock("~/stt/useRunBatch", () => ({ useRunBatch: () => mocks.runBatch }));
vi.mock("@anlg/plugin-shortcut", () => ({
  commands: { configure: mocks.configure, setActive: mocks.setActive },
  events: {
    shortcutEvent: {
      listen: vi.fn(async (listener) => {
        mocks.listener = listener;
        return mocks.unlisten;
      }),
    },
  },
}));
vi.mock("@anlg/plugin-dictation", () => ({
  commands: {
    show: mocks.show,
    hide: mocks.hide,
    setPhase: mocks.setPhase,
    captureTarget: mocks.captureTarget,
    startRecording: mocks.startRecording,
    stopRecording: mocks.stopRecording,
    cancelRecording: mocks.cancelRecording,
    discardRecording: mocks.discardRecording,
    insertText: mocks.insertText,
  },
}));
vi.mock("@anlg/plugin-permissions", () => ({
  commands: { checkPermission: mocks.checkPermission },
}));
vi.mock("@anlg/plugin-transcription", () => ({
  commands: { getCaptureState: mocks.getCaptureState },
}));
vi.mock("@anlg/ui/components/ui/toast", () => ({
  sonnerToast: { error: vi.fn() },
}));

import { DictationLifecycle, useDictationStatus } from "./lifecycle";

describe("dictation access and lifecycle", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.session = { user: { id: "user-1" } };
    mocks.billing = { isPro: true, isReady: true };
    mocks.platform = "macos";
    mocks.settings.dictation_enabled = true;
    mocks.settings.microphone_device = "";
    mocks.meeting = { status: "inactive", loading: false };
    mocks.listener = null;
    for (const fn of [
      mocks.configure,
      mocks.setActive,
      mocks.show,
      mocks.hide,
      mocks.setPhase,
      mocks.startRecording,
      mocks.cancelRecording,
      mocks.discardRecording,
      mocks.insertText,
    ])
      fn.mockResolvedValue({ status: "ok", data: null });
    mocks.captureTarget.mockResolvedValue({
      status: "ok",
      data: "focused-field",
    });
    mocks.checkPermission.mockResolvedValue({
      status: "ok",
      data: "authorized",
    });
    mocks.getCaptureState.mockResolvedValue({ status: "ok", data: "inactive" });
    mocks.stopRecording.mockResolvedValue({
      status: "ok",
      data: { filePath: "/tmp/dictation.wav" },
    });
    mocks.stopTranscription.mockResolvedValue(undefined);
    mocks.runBatch.mockImplementation(async (_path, options) =>
      options.handlePersist([{ text: "Hello", start_ms: 0 }]),
    );
    useDictationStatus.setState({
      ready: false,
      phase: "idle",
      lastTranscript: "",
      error: null,
      retry: 0,
      cancel: null,
    });
  });

  it.each(["free", "loading", "signed-out", "disabled", "meeting"])(
    "does not register shortcuts when %s",
    async (condition) => {
      if (condition === "free") mocks.billing.isPro = false;
      if (condition === "loading") mocks.billing.isReady = false;
      if (condition === "signed-out") mocks.session = null;
      if (condition === "disabled") mocks.settings.dictation_enabled = false;
      if (condition === "meeting") mocks.meeting.status = "active";
      render(<DictationLifecycle />);
      await act(async () => {});
      expect(mocks.configure).not.toHaveBeenCalled();
      expect(mocks.startRecording).not.toHaveBeenCalled();
    },
  );

  it("registers for Pro access and inserts into the captured field", async () => {
    render(<DictationLifecycle />);
    await waitFor(() => expect(useDictationStatus.getState().ready).toBe(true));
    expect(mocks.configure).toHaveBeenCalledWith("Control+Alt+Space");
    await act(async () => {
      mocks.listener?.({ payload: { type: "pressed" } });
    });
    await act(async () => {
      mocks.listener?.({ payload: { type: "released" } });
    });
    await waitFor(() =>
      expect(mocks.insertText).toHaveBeenCalledWith("focused-field", "Hello"),
    );
    expect(mocks.startRecording).toHaveBeenCalledWith(
      null,
      expect.stringMatching(/^system-dictation-/u),
    );
    expect(mocks.discardRecording).toHaveBeenCalledWith("/tmp/dictation.wav");
  });

  it("aborts and removes shortcuts when paid access is lost during recording", async () => {
    const view = render(<DictationLifecycle />);
    await waitFor(() => expect(useDictationStatus.getState().ready).toBe(true));
    await act(async () => {
      mocks.listener?.({ payload: { type: "pressed" } });
    });
    await waitFor(() =>
      expect(useDictationStatus.getState().phase).toBe("recording"),
    );
    mocks.billing.isPro = false;
    view.rerender(<DictationLifecycle />);
    await waitFor(() => expect(mocks.configure).toHaveBeenLastCalledWith(null));
    expect(mocks.cancelRecording).toHaveBeenCalledWith(
      expect.stringMatching(/^system-dictation-/u),
    );
    expect(mocks.insertText).not.toHaveBeenCalled();
    expect(useDictationStatus.getState().lastTranscript).toBe("");
  });

  it("reports missing microphone permission without capturing a target or audio", async () => {
    mocks.checkPermission.mockResolvedValue({ status: "ok", data: "denied" });
    render(<DictationLifecycle />);
    await waitFor(() => expect(useDictationStatus.getState().ready).toBe(true));
    await act(async () => {
      mocks.listener?.({ payload: { type: "pressed" } });
    });
    expect(mocks.captureTarget).not.toHaveBeenCalled();
    expect(mocks.startRecording).not.toHaveBeenCalled();
    expect(useDictationStatus.getState().error).toMatch(
      /microphone permission/u,
    );
  });

  it.each(["windows", "linux"])(
    "opens the selected microphone on %s without probing a potentially missing default device",
    async (platform) => {
      mocks.platform = platform;
      mocks.settings.microphone_device = "USB microphone";
      mocks.checkPermission.mockResolvedValue({ status: "ok", data: "denied" });
      render(<DictationLifecycle />);
      await waitFor(() =>
        expect(useDictationStatus.getState().ready).toBe(true),
      );
      await act(async () => {
        mocks.listener?.({ payload: { type: "pressed" } });
      });
      expect(mocks.checkPermission).not.toHaveBeenCalled();
      expect(mocks.startRecording).toHaveBeenCalledWith(
        "USB microphone",
        expect.stringMatching(/^system-dictation-/u),
      );
    },
  );
});
