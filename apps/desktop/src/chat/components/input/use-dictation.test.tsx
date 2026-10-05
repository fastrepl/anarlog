import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { StrictMode, type ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  cancelRecording: vi.fn(),
  discardRecording: vi.fn(),
  getCaptureState: vi.fn(),
  runBatch: vi.fn(),
  startRecording: vi.fn(),
  stopRecording: vi.fn(),
  toastError: vi.fn(),
  toastWarning: vi.fn(),
  useRunBatch: vi.fn(),
}));

vi.mock("@anlg/plugin-dictation", () => ({
  commands: {
    cancelRecording: mocks.cancelRecording,
    discardRecording: mocks.discardRecording,
    startRecording: mocks.startRecording,
    stopRecording: mocks.stopRecording,
  },
}));

vi.mock("@anlg/plugin-transcription", () => ({
  commands: {
    getCaptureState: mocks.getCaptureState,
  },
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  toast: {
    error: mocks.toastError,
    warning: mocks.toastWarning,
  },
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: () => "Built-in Microphone",
}));

vi.mock("~/stt/useRunBatch", () => ({
  useRunBatch: (sessionId: string) => {
    mocks.useRunBatch(sessionId);
    return mocks.runBatch;
  },
}));

import type { ChatEditorHandle } from "@anlg/editor/chat";

import { useDictation } from "./use-dictation";

import { useTabs } from "~/store/zustand/tabs";

describe("useDictation", () => {
  beforeEach(async () => {
    cleanup();
    await Promise.resolve();
    vi.useRealTimers();
    vi.clearAllMocks();
    useTabs.setState({ chatMode: "FloatingOpen" });
    mocks.getCaptureState.mockResolvedValue({
      status: "ok",
      data: "inactive",
    });
    mocks.startRecording.mockResolvedValue({ status: "ok", data: null });
    mocks.stopRecording.mockResolvedValue({
      status: "ok",
      data: { filePath: "/tmp/voice.wav", durationMs: 1_200 },
    });
    mocks.discardRecording.mockResolvedValue({ status: "ok", data: null });
    mocks.cancelRecording.mockResolvedValue({ status: "ok", data: null });
    mocks.runBatch.mockImplementation(async (_filePath, options) => {
      options.handlePersist([
        {
          text: " Hello",
          start_ms: 0,
          end_ms: 500,
          channel: 0,
        },
        {
          text: " world.",
          start_ms: 500,
          end_ms: 1_000,
          channel: 0,
        },
      ]);
    });
  });

  it("records, transcribes, inserts, and removes temporary audio", async () => {
    const editor = {
      focus: vi.fn(() => true),
      insertText: vi.fn(),
    } as unknown as ChatEditorHandle;
    const editorRef = { current: editor };
    const { result } = renderHook(() => useDictation({ editorRef }));

    expect(mocks.useRunBatch).toHaveBeenCalledWith(
      expect.stringMatching(/^chat-dictation-/u),
    );
    expect(mocks.useRunBatch).not.toHaveBeenCalledWith("chat-1");

    await act(async () => {
      await result.current.start();
    });
    expect(result.current.phase).toBe("recording");
    expect(mocks.startRecording).toHaveBeenCalledWith(
      "Built-in Microphone",
      mocks.useRunBatch.mock.calls[0][0],
    );

    await act(async () => {
      await result.current.stop();
    });

    expect(mocks.runBatch).toHaveBeenCalledWith(
      "/tmp/voice.wav",
      expect.objectContaining({
        deferAudioFinalization: true,
        notifyOnCompletion: false,
        numSpeakers: 1,
      }),
    );
    expect(editor.insertText).toHaveBeenCalledWith("Hello world.");
    expect(mocks.discardRecording).toHaveBeenCalledWith("/tmp/voice.wav");
    expect(result.current.phase).toBe("idle");
  });

  it("does not compete with an active meeting recording", async () => {
    mocks.getCaptureState.mockResolvedValue({
      status: "ok",
      data: "active",
    });
    const editorRef = { current: null };
    const { result } = renderHook(() => useDictation({ editorRef }));

    await act(async () => {
      await result.current.start();
    });

    expect(mocks.startRecording).not.toHaveBeenCalled();
    expect(mocks.toastWarning).toHaveBeenCalledWith(
      "Voice input is unavailable while Anarlog is recording a meeting.",
    );
  });

  it("stops and transcribes at the recording time limit", async () => {
    vi.useFakeTimers();
    const editor = {
      focus: vi.fn(() => true),
      insertText: vi.fn(),
    } as unknown as ChatEditorHandle;
    const editorRef = { current: editor };
    const { result } = renderHook(() => useDictation({ editorRef }));

    await act(async () => {
      await result.current.start();
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5 * 60 * 1_000);
    });

    expect(mocks.stopRecording).toHaveBeenCalledOnce();
    expect(mocks.runBatch).toHaveBeenCalledOnce();
    expect(editor.insertText).toHaveBeenCalledWith("Hello world.");
    expect(result.current.phase).toBe("idle");
  });

  it("remains available after StrictMode replays mount cleanup", async () => {
    const editorRef = { current: null };
    const { result } = renderHook(() => useDictation({ editorRef }), {
      wrapper: ({ children }: { children: ReactNode }) => (
        <StrictMode>{children}</StrictMode>
      ),
    });

    await act(async () => {
      await result.current.start();
    });

    expect(result.current.phase).toBe("recording");
    expect(mocks.startRecording).toHaveBeenCalledOnce();
  });

  it("cancels a recording that fails to start cleanly", async () => {
    mocks.startRecording.mockResolvedValue({
      status: "error",
      error: "microphone unavailable",
    });
    const editorRef = { current: null };
    const { result } = renderHook(() => useDictation({ editorRef }));

    await act(async () => {
      await result.current.start();
    });

    expect(mocks.cancelRecording).toHaveBeenCalledOnce();
    expect(result.current.phase).toBe("idle");
  });

  it("cancels an active recording when the chat input unmounts", async () => {
    const editorRef = { current: null };
    const { result, unmount } = renderHook(() => useDictation({ editorRef }));

    await act(async () => {
      await result.current.start();
    });
    unmount();

    await waitFor(() => {
      expect(mocks.cancelRecording).toHaveBeenCalledOnce();
    });
  });

  it("waits for native cancellation before starting after chat reopens", async () => {
    let releaseCancellation!: () => void;
    const cancellation = new Promise<void>((resolve) => {
      releaseCancellation = resolve;
    });
    let capturing = false;
    mocks.startRecording.mockImplementation(async () => {
      if (capturing) {
        return { status: "error", error: "AlreadyRecording" };
      }
      capturing = true;
      return { status: "ok", data: null };
    });
    mocks.cancelRecording.mockImplementation(async () => {
      await cancellation;
      capturing = false;
      return { status: "ok", data: null };
    });
    const { result } = renderHook(() =>
      useDictation({ editorRef: { current: null } }),
    );
    await act(async () => {
      await result.current.start();
    });
    act(() => useTabs.setState({ chatMode: "FloatingClosed" }));
    act(() => useTabs.setState({ chatMode: "FloatingOpen" }));
    let restarted!: Promise<void>;
    await act(async () => {
      restarted = result.current.start();
    });
    expect(result.current.phase).toBe("starting");
    await act(async () => {
      releaseCancellation();
      await restarted;
    });
    expect(result.current.phase).toBe("recording");
    expect(capturing).toBe(true);
    expect(mocks.toastError).not.toHaveBeenCalled();
  });

  it("keeps transcription busy across close and reopen until the old batch settles", async () => {
    let finishBatch!: () => void;
    const pending = new Promise<void>((resolve) => {
      finishBatch = resolve;
    });
    mocks.runBatch.mockImplementationOnce(async (_path, options) => {
      await pending;
      options.handlePersist([
        { text: "Discarded", start_ms: 0, end_ms: 1, channel: 0 },
      ]);
    });
    const editor = {
      focus: vi.fn(),
      insertText: vi.fn(),
    } as unknown as ChatEditorHandle;
    const { result } = renderHook(() =>
      useDictation({ editorRef: { current: editor } }),
    );
    await act(async () => {
      await result.current.start();
    });
    let stopped!: Promise<void>;
    await act(async () => {
      stopped = result.current.stop();
    });
    act(() => useTabs.setState({ chatMode: "FloatingClosed" }));
    act(() => useTabs.setState({ chatMode: "FloatingOpen" }));
    await act(async () => {
      await result.current.start();
    });
    expect(result.current.phase).toBe("transcribing");
    await act(async () => {
      finishBatch();
      await stopped;
    });
    expect(result.current.phase).toBe("idle");
    expect(editor.insertText).not.toHaveBeenCalled();
    await act(async () => {
      await result.current.start();
      await result.current.stop();
    });
    expect(editor.insertText).toHaveBeenCalledWith("Hello world.");
  });
});
