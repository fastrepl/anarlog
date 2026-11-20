import { act, cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  session: { user: { id: "user-1" } } as { user: { id: string } } | null,
  billing: { isPro: true, isReady: true },
  platform: "macos",
  settings: {
    dictation_enabled: true,
    dictation_shortcut: "Control+Alt+Space",
    dictation_hands_free: false,
    dictation_live_preview: false,
  } as Record<string, unknown>,
  meeting: { status: "inactive", loading: false },
  configure: vi.fn(),
  setActive: vi.fn(),
  captureTarget: vi.fn(),
  checkPermission: vi.fn(),
  getCaptureState: vi.fn(),
  stream: vi.fn(),
  unlisten: vi.fn(),
  listener: null as ((event: { payload: { type: string } }) => void) | null,
  gesture: null as
    | ((event: { payload: { type: string; latched?: boolean } }) => void)
    | null,
  configureGestures: vi.fn(),
  insertDictation: vi.fn(),
  releaseDictationTarget: vi.fn(),
  retireDictations: vi.fn(),
  cleaned: false,
  native: null as unknown as FakeNative,
  engineCommands: null as unknown as ReturnType<typeof fakeDictationCommands>,
}));
vi.mock("./planner", () => ({
  useDictationPlanner: () => ({
    microphone: () => "",
    options: () => ({ context: false, learning: false, voiceEdit: false }),
    used: async () => {},
    kept: async () => {},
    plan: async () => ({
      policy: { provider: "deepgram", model: "nova-3", localOnly: false },
      microphoneDevice: null,
      recovery: false,
      ducking: false,
      livePreview: mocks.settings.dictation_live_preview,
      stream: () => mocks.stream(),
      transcribe: async () => "Batched words.",
      refine: async ({ text }: { text: string }) =>
        mocks.cleaned
          ? { type: "cleaned", text: `${text} Cleaned.` }
          : { type: "raw", text, reason: "cleanupOff" },
    }),
  }),
}));
vi.mock("~/analytics", () => ({ trackAnalyticsEvent: () => {} }));
vi.mock("./panel", () => ({ waitForDictationPanel: vi.fn(async () => {}) }));
vi.mock("@tauri-apps/api/core", () => ({
  Channel: class {
    constructor(public onmessage: (event: unknown) => void) {}
  },
}));
vi.mock("~/auth", () => ({
  useAuth: () => ({ session: mocks.session }),
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
    select({ live: mocks.meeting }),
}));
vi.mock("@anlg/plugin-shortcut", () => ({
  commands: {
    configure: mocks.configure,
    configureGestures: mocks.configureGestures,
    setActive: mocks.setActive,
  },
  events: {
    shortcutEvent: {
      listen: vi.fn(async (listener) => {
        mocks.listener = listener;
        return mocks.unlisten;
      }),
    },
    dictationGesture: {
      listen: vi.fn(async (listener) => {
        mocks.gesture = listener;
        return mocks.unlisten;
      }),
    },
  },
}));
vi.mock("@anlg/plugin-dictation", () => ({
  commands: {
    captureTarget: mocks.captureTarget,
    insertDictation: mocks.insertDictation,
    releaseDictationTarget: mocks.releaseDictationTarget,
    retireDictations: mocks.retireDictations,
    ...Object.fromEntries(
      [
        "beginDictation",
        "attachDictationStream",
        "finishDictation",
        "completeDictation",
        "releaseDictation",
        "cancelDictation",
        "saveDictationRecoveryText",
      ].map((name) => [
        name,
        (...args: unknown[]) =>
          (
            mocks.engineCommands[name as keyof typeof mocks.engineCommands] as (
              ...args: unknown[]
            ) => unknown
          )(...args),
      ]),
    ),
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

import { DictationLifecycle, waitForDictationCleanup } from "./lifecycle";
import { useDictationStatus } from "./state";
import { FakeNative, fakeDictationCommands } from "./test-utils";

describe("system dictation", () => {
  afterEach(async () => {
    cleanup();
    await waitForDictationCleanup();
  });

  beforeEach(() => {
    vi.clearAllMocks();
    useDictationStatus.setState({ capturingShortcut: false });
    mocks.session = { user: { id: "user-1" } };
    mocks.billing = { isPro: true, isReady: true };
    mocks.platform = "macos";
    mocks.settings.dictation_live_preview = false;
    mocks.settings.dictation_enabled = true;
    mocks.settings.dictation_shortcut = "Control+Alt+Space";
    mocks.settings.dictation_hands_free = false;
    mocks.meeting = { status: "inactive", loading: false };
    mocks.listener = null;
    mocks.gesture = null;
    delete mocks.settings.dictation_context_exclusions;
    mocks.cleaned = false;
    mocks.native = new FakeNative();
    mocks.engineCommands = fakeDictationCommands(mocks.native);
    mocks.stream.mockResolvedValue(null);
    mocks.configureGestures.mockResolvedValue({
      status: "ok",
      data: { spaceLatch: true, chordCancel: true },
    });
    mocks.insertDictation.mockImplementation(async (_target, request) => ({
      status: "ok",
      data: { type: "applied", verified: true, text: request.text },
    }));
    mocks.retireDictations.mockResolvedValue({ status: "ok", data: null });
    mocks.releaseDictationTarget.mockResolvedValue({
      status: "ok",
      data: null,
    });
    for (const fn of [mocks.configure, mocks.setActive])
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
    useDictationStatus.setState({
      ready: false,
      phase: "idle",
      lastTranscript: "",
      error: null,
      retry: 0,
      cancel: null,
      finish: null,
    });
  });

  const gesture = (type: string, latched?: boolean) =>
    act(async () => {
      mocks.gesture?.({ payload: { type, latched } });
    });

  const ready = () =>
    waitFor(() => expect(useDictationStatus.getState().ready).toBe(true));

  const phase = (expected: string) =>
    waitFor(() => expect(useDictationStatus.getState().phase).toBe(expected));

  /** The update channel of the dictation that is open natively now. */
  const updates = () =>
    [...mocks.native.ops.values()].find((op) => op.state === "open")!.onUpdate;

  it("retains the last transcript through reconfiguration and clears it when disabled", async () => {
    const { rerender } = render(<DictationLifecycle />);
    await ready();
    useDictationStatus.setState({ lastTranscript: "Recover this text" });
    mocks.settings.dictation_shortcut = "Control+Alt+D";
    rerender(<DictationLifecycle />);
    await waitFor(() =>
      expect(mocks.configureGestures).toHaveBeenLastCalledWith(
        "Control+Alt+D",
        expect.anything(),
      ),
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
    await ready();
    act(() => useDictationStatus.setState({ capturingShortcut: true }));
    await waitFor(() =>
      expect(mocks.configureGestures).toHaveBeenLastCalledWith(
        null,
        expect.anything(),
      ),
    );
    expect(useDictationStatus.getState().ready).toBe(false);
    act(() => useDictationStatus.setState({ capturingShortcut: false }));
    await waitFor(() =>
      expect(mocks.configureGestures).toHaveBeenLastCalledWith(
        "Control+Alt+Space",
        expect.anything(),
      ),
    );
    expect(mocks.native.ops.size).toBe(0);
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
      expect(mocks.configureGestures).not.toHaveBeenCalled();
      expect(mocks.configure).not.toHaveBeenCalled();
    },
  );

  it("inserts a held dictation into the captured field with guarded fitting", async () => {
    render(<DictationLifecycle />);
    await ready();
    expect(mocks.configure).not.toHaveBeenCalled();
    await gesture("prime");
    await gesture("start", false);
    await phase("recording");
    await gesture("stop");
    await phase("idle");

    const [[field, request]] = mocks.insertDictation.mock.calls;
    expect(field).toBe("focused-field");
    expect(request).toEqual({
      operationId: expect.any(String),
      editId: request.operationId,
      text: "Batched words.",
      mode: { type: "fit", canJoin: true },
      exclusions: { apps: [], websites: [] },
    });
    expect(useDictationStatus.getState().lastTranscript).toBe("Batched words.");
    expect(mocks.native.openOps).toBe(0);
    expect(mocks.releaseDictationTarget).toHaveBeenCalledWith("focused-field");
  });

  it("starts and finishes a hands-free dictation with single presses, and cancels it", async () => {
    mocks.settings.dictation_hands_free = true;
    render(<DictationLifecycle />);
    await ready();
    expect(mocks.configure).toHaveBeenCalledWith("Control+Alt+Space");
    expect(mocks.configureGestures).not.toHaveBeenCalled();

    await act(async () => mocks.listener?.({ payload: { type: "pressed" } }));
    await phase("recording");
    // Releasing the key does not finish a hands-free dictation.
    await act(async () => mocks.listener?.({ payload: { type: "released" } }));
    expect(useDictationStatus.getState().phase).toBe("recording");
    await act(async () => mocks.listener?.({ payload: { type: "pressed" } }));
    await phase("idle");
    expect(mocks.insertDictation).toHaveBeenCalledOnce();

    await act(async () => mocks.listener?.({ payload: { type: "pressed" } }));
    await phase("recording");
    await act(async () => mocks.listener?.({ payload: { type: "cancelled" } }));
    await phase("idle");
    expect(mocks.insertDictation).toHaveBeenCalledOnce();
    expect(mocks.native.openOps).toBe(0);
  });

  it("aborts and removes shortcuts when paid access is lost during recording", async () => {
    const view = render(<DictationLifecycle />);
    await ready();
    await gesture("start", true);
    await phase("recording");
    mocks.billing.isPro = false;
    view.rerender(<DictationLifecycle />);
    await waitFor(() =>
      expect(mocks.configureGestures).toHaveBeenLastCalledWith(
        null,
        expect.anything(),
      ),
    );
    await waitFor(() => expect(mocks.native.openOps).toBe(0));
    expect(mocks.native.streamedAny).toBe(false);
    expect(mocks.insertDictation).not.toHaveBeenCalled();
    expect(useDictationStatus.getState().lastTranscript).toBe("");
  });

  it("reports missing microphone permission without capturing a target or audio", async () => {
    mocks.checkPermission.mockResolvedValue({ status: "ok", data: "denied" });
    render(<DictationLifecycle />);
    await ready();
    await gesture("prime");
    await gesture("start", false);
    await waitFor(() =>
      expect(useDictationStatus.getState().error).toMatch(
        /microphone permission/u,
      ),
    );
    expect(useDictationStatus.getState().owner).toBeNull();
    expect(mocks.captureTarget).not.toHaveBeenCalled();
    expect(mocks.native.ops.size).toBe(0);
  });

  it.each(["windows", "linux"])(
    "opens the microphone on %s without asking for macOS permission",
    async (platform) => {
      mocks.platform = platform;
      mocks.checkPermission.mockResolvedValue({ status: "ok", data: "denied" });
      render(<DictationLifecycle />);
      await ready();
      await gesture("start", true);
      await phase("recording");
      expect(mocks.checkPermission).not.toHaveBeenCalled();
      expect(mocks.native.openOps).toBe(1);
    },
  );

  it("shows live words without inserting until finish, then ignores stale preview updates", async () => {
    mocks.settings.dictation_live_preview = true;
    render(<DictationLifecycle />);
    await ready();
    await gesture("start", true);
    await phase("recording");
    const update = updates();
    await act(async () =>
      update({ type: "transcript", text: "Hello", partial: "there" }),
    );
    expect(useDictationStatus.getState()).toMatchObject({
      text: "Hello",
      partial: "there",
      expanded: true,
    });
    expect(mocks.insertDictation).not.toHaveBeenCalled();
    await act(async () => useDictationStatus.getState().finish?.());
    await phase("idle");
    expect(mocks.insertDictation).toHaveBeenCalledOnce();
    await act(async () =>
      update({ type: "transcript", text: "Stale", partial: "" }),
    );
    expect(useDictationStatus.getState()).toMatchObject({
      owner: null,
      text: "",
      lastTranscript: "Batched words.",
    });
  });

  it("drops preview updates after cancellation and neither transcribes nor inserts", async () => {
    mocks.settings.dictation_live_preview = true;
    render(<DictationLifecycle />);
    await ready();
    await gesture("start", true);
    await phase("recording");
    const update = updates();
    await act(async () => useDictationStatus.getState().cancel?.());
    await phase("idle");
    await act(async () =>
      update({ type: "transcript", text: "Cancelled speech", partial: "" }),
    );
    expect(useDictationStatus.getState().text).toBe("");
    expect(mocks.insertDictation).not.toHaveBeenCalled();
    expect(mocks.native.openOps).toBe(0);
  });

  it("captures the destination before waiting for the live stream's token", async () => {
    mocks.settings.dictation_live_preview = true;
    let token!: (stream: null) => void;
    mocks.stream.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          token = resolve;
        }),
    );
    render(<DictationLifecycle />);
    await ready();
    await gesture("start", true);
    await waitFor(() => expect(mocks.stream).toHaveBeenCalledOnce());
    expect(mocks.captureTarget).toHaveBeenCalledOnce();
    expect(mocks.captureTarget.mock.invocationCallOrder[0]).toBeLessThan(
      mocks.stream.mock.invocationCallOrder[0],
    );
    mocks.captureTarget.mockResolvedValue({
      status: "ok",
      data: "different-field",
    });
    await act(async () => token(null));
    await gesture("stop");
    await phase("idle");
    expect(mocks.captureTarget).toHaveBeenCalledOnce();
    expect(mocks.insertDictation.mock.calls[0]![0]).toBe("focused-field");
  });

  it("keeps recording when no live stream is available", async () => {
    mocks.settings.dictation_live_preview = true;
    render(<DictationLifecycle />);
    await ready();
    await gesture("start", true);
    await waitFor(() =>
      expect(useDictationStatus.getState()).toMatchObject({
        phase: "recording",
        previewUnavailable: true,
      }),
    );
    expect(mocks.native.streamedAny).toBe(false);
  });

  it("puts back the raw words into the delivered field, with its exclusions, after the tap's own prime", async () => {
    mocks.cleaned = true;
    mocks.settings.dictation_context_exclusions = JSON.stringify({
      apps: ["com.example.bank"],
      websites: [],
    });
    mocks.captureTarget
      .mockResolvedValueOnce({ status: "ok", data: "field-1" })
      .mockResolvedValueOnce({ status: "ok", data: "field-2" });
    render(<DictationLifecycle />);
    await ready();
    await gesture("start", true);
    await phase("recording");
    await gesture("stop");
    await phase("idle");
    await gesture("prime");
    await waitFor(() => expect(mocks.captureTarget).toHaveBeenCalledTimes(2));
    await gesture("unprime");
    await gesture("tap");

    await waitFor(() => expect(mocks.insertDictation).toHaveBeenCalledTimes(2));
    const [[, delivery], [field, restore]] = mocks.insertDictation.mock.calls;
    expect(field).toBe("field-1");
    expect(restore).toEqual({
      operationId: delivery.operationId,
      editId: `${delivery.operationId}:raw`,
      text: "Batched words.",
      mode: { type: "restore", editId: delivery.operationId },
      exclusions: { apps: ["com.example.bank"], websites: [] },
    });
    await waitFor(() =>
      expect(
        mocks.releaseDictationTarget.mock.calls
          .map(([target]) => target)
          .sort(),
      ).toEqual(["field-1", "field-2"]),
    );
  });

  it("lets a tap open and close the microphone without inserting", async () => {
    render(<DictationLifecycle />);
    await ready();
    await gesture("prime");
    await gesture("unprime");
    await waitFor(() => expect(mocks.native.openOps).toBe(0));

    expect(mocks.native.streamedAny).toBe(false);
    expect(mocks.insertDictation).not.toHaveBeenCalled();
    expect(useDictationStatus.getState().phase).toBe("idle");
    await waitFor(() =>
      expect(mocks.releaseDictationTarget).toHaveBeenCalledWith(
        "focused-field",
      ),
    );
  });

  it("never retries a write that may have landed", async () => {
    mocks.insertDictation.mockResolvedValue({
      status: "ok",
      data: { type: "uncertain" },
    });
    render(<DictationLifecycle />);
    await ready();
    await gesture("start", true);
    await phase("recording");
    await gesture("stop");
    await phase("idle");

    expect(mocks.insertDictation).toHaveBeenCalledTimes(1);
    expect(useDictationStatus.getState()).toMatchObject({
      lastTranscript: "Batched words.",
      error: expect.stringMatching(/Couldn't confirm/u),
    });
  });

  it("keeps a refused insert's text for copying instead of retrying it", async () => {
    mocks.insertDictation.mockResolvedValue({
      status: "ok",
      data: { type: "unchanged", reason: "focusMoved" },
    });
    render(<DictationLifecycle />);
    await ready();
    await gesture("start", true);
    await phase("recording");
    await gesture("stop");
    await phase("idle");

    expect(mocks.insertDictation).toHaveBeenCalledTimes(1);
    expect(useDictationStatus.getState()).toMatchObject({
      lastTranscript: "Batched words.",
      error: expect.stringMatching(/Couldn't insert the text/u),
    });
  });

  it("retires the previous account's dictations on an account switch", async () => {
    const { rerender } = render(<DictationLifecycle />);
    await ready();
    mocks.session = { user: { id: "user-2" } };
    rerender(<DictationLifecycle />);

    await waitFor(() =>
      expect(mocks.retireDictations).toHaveBeenCalledWith({
        type: "account",
        accountId: "user-1",
      }),
    );
  });

  it("retires dictations when a meeting starts", async () => {
    const { rerender } = render(<DictationLifecycle />);
    await ready();
    mocks.meeting = { status: "active", loading: false };
    rerender(<DictationLifecycle />);

    await waitFor(() =>
      expect(mocks.retireDictations).toHaveBeenCalledWith({
        type: "meeting",
      }),
    );
  });
});
