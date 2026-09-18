import { act, cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  isCloudModel: false,
  getSessionForRequest: vi.fn(),
  connection: null as {
    provider: string;
    model: string;
    apiKey: string;
    baseUrl: string;
  } | null,
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
  startSystemRecording: vi.fn(),
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
vi.mock("./panel", () => ({ waitForDictationPanel: vi.fn(async () => {}) }));
vi.mock("@tauri-apps/api/core", () => ({
  Channel: class {
    constructor(public onmessage: (event: unknown) => void) {}
  },
}));
vi.mock("~/stt/useSTTConnection", () => ({
  useSTTConnection: () => ({
    conn: mocks.connection,
    isCloudModel: mocks.isCloudModel,
  }),
}));
vi.mock("~/auth", () => ({
  useAuth: () => ({
    session: mocks.session,
    getSessionForRequest: mocks.getSessionForRequest,
  }),
}));
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
    startSystemRecording: mocks.startSystemRecording,
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
  toast: { error: vi.fn() },
}));

import {
  DictationLifecycle,
  useDictationStatus,
  waitForDictationCleanup,
} from "./lifecycle";

describe("dictation access and lifecycle", () => {
  afterEach(async () => {
    cleanup();
    await waitForDictationCleanup();
  });
  it("retains the last transcript through reconfiguration and clears it when disabled", async () => {
    const { rerender } = render(<DictationLifecycle />);
    await waitFor(() => expect(useDictationStatus.getState().ready).toBe(true));
    useDictationStatus.setState({ lastTranscript: "Recover this text" });
    mocks.settings.dictation_shortcut = "Control+Alt+D";
    rerender(<DictationLifecycle />);
    await waitFor(() =>
      expect(mocks.configure).toHaveBeenLastCalledWith("Control+Alt+D"),
    );
    expect(useDictationStatus.getState().lastTranscript).toBe(
      "Recover this text",
    );
    mocks.settings.dictation_enabled = false;
    rerender(<DictationLifecycle />);
    expect(useDictationStatus.getState().lastTranscript).toBe("");
  });

  it("unregisters dictation while capturing a shortcut and restores it afterwards", async () => {
    render(<DictationLifecycle />);
    await waitFor(() => expect(useDictationStatus.getState().ready).toBe(true));
    act(() => useDictationStatus.setState({ capturingShortcut: true }));
    await waitFor(() => expect(mocks.configure).toHaveBeenLastCalledWith(null));
    expect(useDictationStatus.getState().ready).toBe(false);
    act(() => useDictationStatus.setState({ capturingShortcut: false }));
    await waitFor(() =>
      expect(mocks.configure).toHaveBeenLastCalledWith("Control+Alt+Space"),
    );
    expect(mocks.startSystemRecording).not.toHaveBeenCalled();
  });

  beforeEach(() => {
    vi.clearAllMocks();
    useDictationStatus.setState({ capturingShortcut: false });
    mocks.session = { user: { id: "user-1" } };
    mocks.billing = { isPro: true, isReady: true };
    mocks.platform = "macos";
    mocks.connection = null;
    mocks.isCloudModel = false;
    mocks.getSessionForRequest.mockResolvedValue({
      access_token: "refreshed-token",
    });
    mocks.settings.dictation_live_preview = false;
    mocks.settings.dictation_enabled = true;
    mocks.settings.dictation_shortcut = "Control+Alt+Space";
    mocks.settings.microphone_device = "";
    mocks.meeting = { status: "inactive", loading: false };
    mocks.listener = null;
    for (const fn of [
      mocks.configure,
      mocks.setActive,
      mocks.show,
      mocks.hide,
      mocks.setPhase,
      mocks.startSystemRecording,
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
      expect(mocks.startSystemRecording).not.toHaveBeenCalled();
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
    expect(mocks.startSystemRecording).toHaveBeenCalledWith(
      null,
      expect.stringMatching(/^system-dictation-/u),
      null,
      expect.anything(),
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
    expect(useDictationStatus.getState().owner).toBeNull();
    expect(mocks.captureTarget).not.toHaveBeenCalled();
    expect(mocks.startSystemRecording).not.toHaveBeenCalled();
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
      expect(mocks.startSystemRecording).toHaveBeenCalledWith(
        "USB microphone",
        expect.stringMatching(/^system-dictation-/u),
        null,
        expect.anything(),
      );
    },
  );
  it("shows live words without inserting until finish, then ignores stale preview updates", async () => {
    mocks.settings.dictation_live_preview = true;
    mocks.connection = {
      provider: "deepgram",
      model: "nova-3",
      apiKey: "test",
      baseUrl: "https://api.deepgram.com",
    };
    render(<DictationLifecycle />);
    await waitFor(() => expect(useDictationStatus.getState().ready).toBe(true));
    await act(async () => {
      mocks.listener?.({ payload: { type: "pressed" } });
    });
    const channel = mocks.startSystemRecording.mock.calls[0]![3];
    expect(mocks.startSystemRecording.mock.calls[0]![2]).toEqual(
      expect.objectContaining({ provider: "deepgram", apiKey: "test" }),
    );
    await act(async () => {
      channel.onmessage({
        type: "transcript",
        text: "Hello",
        partial: "there",
      });
    });
    expect(useDictationStatus.getState()).toMatchObject({
      text: "Hello",
      partial: "there",
      expanded: true,
    });
    expect(mocks.insertText).not.toHaveBeenCalled();
    await act(async () => {
      useDictationStatus.getState().finish?.();
    });
    await waitFor(() =>
      expect(mocks.insertText).toHaveBeenCalledWith("focused-field", "Hello"),
    );
    await act(async () => {
      channel.onmessage({ type: "transcript", text: "Stale", partial: "" });
    });
    expect(useDictationStatus.getState()).toMatchObject({
      owner: null,
      text: "",
      lastTranscript: "Hello",
    });
  });

  it("drops preview updates after cancellation and does not transcribe or insert", async () => {
    render(<DictationLifecycle />);
    await waitFor(() => expect(useDictationStatus.getState().ready).toBe(true));
    await act(async () => {
      mocks.listener?.({ payload: { type: "pressed" } });
    });
    const channel = mocks.startSystemRecording.mock.calls[0]![3];
    await act(async () => {
      useDictationStatus.getState().cancel?.();
    });
    await waitFor(() =>
      expect(useDictationStatus.getState().phase).toBe("idle"),
    );
    await act(async () => {
      channel.onmessage({
        type: "transcript",
        text: "Cancelled speech",
        partial: "",
      });
    });
    expect(useDictationStatus.getState().text).toBe("");
    expect(mocks.runBatch).not.toHaveBeenCalled();
    expect(mocks.insertText).not.toHaveBeenCalled();
  });
  it("keeps local preview local without requesting a cloud token", async () => {
    mocks.settings.dictation_live_preview = true;
    mocks.connection = {
      provider: "anarlog",
      model: "soniqo-parakeet-streaming",
      apiKey: "",
      baseUrl: "http://127.0.0.1:1234",
    };
    render(<DictationLifecycle />);
    await waitFor(() => expect(useDictationStatus.getState().ready).toBe(true));
    await act(async () => {
      mocks.listener?.({ payload: { type: "pressed" } });
    });
    expect(mocks.getSessionForRequest).not.toHaveBeenCalled();
    expect(mocks.startSystemRecording.mock.calls[0]![2]).toEqual(
      expect.objectContaining({ baseUrl: "http://127.0.0.1:1234", apiKey: "" }),
    );
  });

  it("captures the destination before waiting for a preview token", async () => {
    mocks.settings.dictation_live_preview = true;
    mocks.isCloudModel = true;
    mocks.connection = {
      provider: "anarlog",
      model: "cloud",
      apiKey: "stale",
      baseUrl: "https://api.anarlog.so/stt",
    };
    let finishAuth!: (session: { access_token: string }) => void;
    mocks.getSessionForRequest.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finishAuth = resolve;
        }),
    );
    render(<DictationLifecycle />);
    await waitFor(() => expect(useDictationStatus.getState().ready).toBe(true));
    await act(async () => {
      mocks.listener?.({ payload: { type: "pressed" } });
    });
    expect(mocks.getSessionForRequest).toHaveBeenCalledOnce();
    expect(mocks.captureTarget).toHaveBeenCalledOnce();
    expect(mocks.captureTarget.mock.invocationCallOrder[0]).toBeLessThan(
      mocks.getSessionForRequest.mock.invocationCallOrder[0],
    );
    mocks.captureTarget.mockResolvedValue({
      status: "ok",
      data: "different-field",
    });
    await act(async () => {
      finishAuth({ access_token: "fresh" });
    });
    expect(mocks.captureTarget).toHaveBeenCalledOnce();
    expect(mocks.startSystemRecording).toHaveBeenCalledOnce();
    await act(async () => {
      mocks.listener?.({ payload: { type: "released" } });
    });
    await waitFor(() =>
      expect(mocks.insertText).toHaveBeenCalledWith("focused-field", "Hello"),
    );
  });

  it("keeps recording available when a cloud preview session cannot be refreshed", async () => {
    mocks.settings.dictation_live_preview = true;
    mocks.isCloudModel = true;
    mocks.connection = {
      provider: "anarlog",
      model: "cloud",
      apiKey: "stale-token",
      baseUrl: "https://api.anarlog.so/stt",
    };
    mocks.getSessionForRequest.mockRejectedValue(new Error("offline"));
    render(<DictationLifecycle />);
    await waitFor(() => expect(useDictationStatus.getState().ready).toBe(true));
    await act(async () => {
      mocks.listener?.({ payload: { type: "pressed" } });
    });
    expect(mocks.startSystemRecording.mock.calls[0]![2]).toBeNull();
    expect(useDictationStatus.getState()).toMatchObject({
      phase: "recording",
      previewUnavailable: true,
    });
  });
});
