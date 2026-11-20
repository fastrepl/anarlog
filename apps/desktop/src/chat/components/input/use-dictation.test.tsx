import {
  act,
  cleanup,
  fireEvent,
  render,
  renderHook,
  waitFor,
} from "@testing-library/react";
import { createRef, StrictMode, type ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  getCaptureState: vi.fn(),
  toastError: vi.fn(),
  toastWarning: vi.fn(),
  native: null as unknown as FakeNative,
  engineCommands: null as unknown as ReturnType<typeof fakeDictationCommands>,
  transcribe: null as
    | null
    | ((path: string, signal: AbortSignal) => Promise<string>),
  options: { context: false, learning: false, voiceEdit: false },
  refine: null as unknown as (request: {
    text: string;
    selection: string | null;
    before: string | null;
  }) => Promise<unknown>,
}));

vi.mock("@anlg/plugin-dictation", () => ({
  commands: {
    ...Object.fromEntries(
      [
        "beginDictation",
        "attachDictationStream",
        "finishDictation",
        "completeDictation",
        "releaseDictation",
        "cancelDictation",
        "saveDictationRecoveryText",
        "fitDictationText",
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

vi.mock("@tauri-apps/api/core", () => ({
  Channel: class {
    constructor(public onmessage: (event: unknown) => void) {}
  },
}));

vi.mock("@tauri-apps/plugin-os", () => ({ platform: () => "macos" }));

vi.mock("@anlg/plugin-permissions", () => ({
  commands: {
    checkPermission: async () => ({ status: "ok", data: "authorized" }),
    requestPermission: async () => ({ status: "ok", data: null }),
  },
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({ session: { user: { id: "user-1" } } }),
}));

vi.mock("~/analytics", () => ({ trackAnalyticsEvent: () => {} }));

vi.mock("~/dictation/planner", () => ({
  useDictationPlanner: () => ({
    microphone: () => "",
    options: () => mocks.options,
    used: async () => {},
    kept: async () => {},
    plan: async () => ({
      policy: { provider: "deepgram", model: "nova-3", localOnly: false },
      microphoneDevice: null,
      recovery: false,
      ducking: false,
      livePreview: false,
      stream: async () => null,
      transcribe: (path: string, signal: AbortSignal) =>
        mocks.transcribe
          ? mocks.transcribe(path, signal)
          : Promise.resolve("Batched words."),
      refine: (request: Parameters<typeof mocks.refine>[0]) =>
        mocks.refine(request),
    }),
  }),
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

import { ChatEditor, type ChatEditorHandle } from "@anlg/editor/chat";

import { useDictation } from "./use-dictation";

import { FakeNative, fakeDictationCommands } from "~/dictation/test-utils";
import { useTabs } from "~/store/zustand/tabs";

describe("useDictation", () => {
  beforeEach(async () => {
    cleanup();
    await Promise.resolve();
    vi.useRealTimers();
    vi.clearAllMocks();
    mocks.transcribe = null;
    mocks.options = { context: false, learning: false, voiceEdit: false };
    mocks.refine = async ({ text }) => ({
      type: "raw",
      text,
      reason: "cleanupOff",
    });
    mocks.native = new FakeNative();
    mocks.engineCommands = fakeDictationCommands(mocks.native);
    // Stands in for dictation-core's caret rules on these sentences: a comma right after
    // a period finishes that sentence, and a word after a word takes a space.
    mocks.engineCommands.fitDictationText = async (
      text: string,
      before?: string | null,
      canJoin?: boolean,
    ) =>
      canJoin && before?.endsWith(".") && text.startsWith(", ")
        ? { text, join: true }
        : {
            text: before && !/\s$/u.test(before) ? ` ${text}` : text,
            join: false,
          };
    useTabs.setState({ chatMode: "FloatingOpen" });
    mocks.getCaptureState.mockResolvedValue({
      status: "ok",
      data: "inactive",
    });
  });

  it("stops and transcribes when the recording reaches its time limit", async () => {
    const composer = renderComposer("");
    const { result } = renderHook(() =>
      useDictation({ editorRef: composer.ref }),
    );
    await act(async () => {
      await result.current.start();
    });
    await waitFor(() => expect(result.current.phase).toBe("recording"));
    const [op] = mocks.native.ops.values();
    await act(async () =>
      op!.onUpdate({ type: "captureEnded", reason: "limit" }),
    );
    await waitFor(() => expect(result.current.phase).toBe("idle"));

    expect(composer.text()).toBe("Batched words.");
    expect(mocks.native.openOps).toBe(0);
  });

  it("remains available after StrictMode replays mount cleanup", async () => {
    const { result } = renderHook(
      () => useDictation({ editorRef: { current: null } }),
      {
        wrapper: ({ children }: { children: ReactNode }) => (
          <StrictMode>{children}</StrictMode>
        ),
      },
    );

    await act(async () => {
      await result.current.start();
    });

    await waitFor(() => expect(result.current.phase).toBe("recording"));
    expect(mocks.native.ops.size).toBe(1);
  });

  it("reports a microphone that fails to open as a start failure", async () => {
    mocks.engineCommands.beginDictation = async () => ({
      status: "error",
      error: { code: "capture", message: "device unavailable" },
    });
    const { result } = renderHook(() =>
      useDictation({ editorRef: { current: null } }),
    );

    await act(async () => {
      await result.current.start();
    });
    await waitFor(() => expect(result.current.phase).toBe("idle"));

    expect(mocks.toastError).toHaveBeenCalledWith(
      "Could not start voice input",
      { description: expect.stringMatching(/couldn't open the microphone/iu) },
    );
  });

  it("cancels an active recording when the chat input unmounts", async () => {
    const { result, unmount } = renderHook(() =>
      useDictation({ editorRef: { current: null } }),
    );

    await act(async () => {
      await result.current.start();
    });
    await waitFor(() => expect(mocks.native.openOps).toBe(1));
    unmount();

    await waitFor(() => expect(mocks.native.openOps).toBe(0));
    expect(mocks.native.streamedAny).toBe(false);
  });

  it("inserts the final dictation once without merging it into the following word", async () => {
    const composer = renderComposer("Next.");
    act(() =>
      composer.ref.current!.replaceContent(paragraphs("Next."), "start"),
    );
    const { result } = renderHook(() =>
      useDictation({ editorRef: composer.ref }),
    );

    await dictate(result);

    expect(composer.text()).toBe("Batched words. Next.");
    expect(mocks.native.openOps).toBe(0);
  });

  it("fits a dictation to the caret where it lands, after what was typed meanwhile", async () => {
    const composer = renderComposer("We'll stay in.");
    const transcript = pending<string>();
    mocks.transcribe = () => transcript.promise;
    mocks.refine = async () => ({ type: "cleaned", text: ", if it snows." });
    const { result } = renderHook(() =>
      useDictation({ editorRef: composer.ref }),
    );

    await dictate(result, { settle: false });
    act(() => composer.ref.current!.insertText("Bring tea."));
    await act(async () => transcript.resolve("if it snows"));
    await waitFor(() => expect(result.current.phase).toBe("idle"));

    expect(composer.text()).toBe("We'll stay in. Bring tea, if it snows.");
  });

  it("sends the composer's text to the model only with on-screen context on", async () => {
    const sent: (string | null)[] = [];
    mocks.refine = async (request) => {
      sent.push(request.before);
      return { type: "raw", text: request.text, reason: "cleanupOff" };
    };
    const composer = renderComposer("Draft so far.");
    const { result } = renderHook(() =>
      useDictation({ editorRef: composer.ref }),
    );

    await dictate(result);
    mocks.options = { ...mocks.options, context: true };
    await dictate(result);

    expect(sent).toEqual([null, "Draft so far. Batched words."]);
    expect(composer.text()).toBe("Draft so far. Batched words. Batched words.");
  });

  it("rewrites by voice the selection it started on", async () => {
    mocks.options = { ...mocks.options, voiceEdit: true };
    mocks.refine = async ({ selection }) => ({
      type: "edited",
      text: selection!.replace("Friday", "Monday"),
    });
    const composer = renderComposer("Ship it Friday.");
    composer.selectAll();
    const { result } = renderHook(() =>
      useDictation({ editorRef: composer.ref }),
    );

    await dictate(result);

    expect(composer.text()).toBe("Ship it Monday.");
  });

  it.each([
    [
      "its selection moved",
      (composer: Composer) =>
        act(() =>
          composer.ref.current!.replaceContent(
            paragraphs("Ship it Friday."),
            "end",
          ),
        ),
      "Ship it Friday.",
    ],
    [
      "it was edited",
      (composer: Composer) =>
        act(() => composer.ref.current!.insertText("Today.")),
      "Today.",
    ],
    [
      "a new editor took its place",
      (composer: Composer) => composer.remount(),
      "Ship it Friday.",
    ],
  ])(
    "leaves the composer as it is when %s while the edit was pending",
    async (_, change, expected) => {
      mocks.options = { ...mocks.options, voiceEdit: true };
      const reply = pending<unknown>();
      mocks.refine = () => reply.promise;
      const composer = renderComposer("Ship it Friday.");
      composer.selectAll();
      const { result } = renderHook(() =>
        useDictation({ editorRef: composer.ref }),
      );

      await dictate(result, { settle: false });
      change(composer);
      await act(async () =>
        reply.resolve({ type: "edited", text: "Ship it Monday." }),
      );
      await waitFor(() => expect(result.current.phase).toBe("idle"));

      expect(composer.text()).toBe(expected);
      expect(mocks.toastError).toHaveBeenCalledWith(expect.any(String), {
        description: expect.stringMatching(/couldn't edit the selection/iu),
      });
      expect(mocks.native.openOps).toBe(0);
    },
  );

  it("drops a late transcript after chat closes", async () => {
    const transcript = pending<string>();
    mocks.transcribe = () => transcript.promise;
    const composer = renderComposer("");
    const { result } = renderHook(() =>
      useDictation({ editorRef: composer.ref }),
    );

    await dictate(result, { settle: false });
    act(() => useTabs.setState({ chatMode: "FloatingClosed" }));
    await act(async () => transcript.resolve("Too late."));
    await waitFor(() => expect(result.current.phase).toBe("idle"));

    expect(composer.text()).toBe("");
    expect(mocks.native.openOps).toBe(0);
  });

  it("does not open the microphone during a meeting", async () => {
    mocks.getCaptureState.mockResolvedValue({ status: "ok", data: "active" });
    const { result } = renderHook(() =>
      useDictation({ editorRef: { current: null } }),
    );

    await act(async () => {
      await result.current.start();
    });
    await waitFor(() => expect(result.current.phase).toBe("idle"));

    expect(mocks.native.ops.size).toBe(0);
    expect(mocks.toastWarning).toHaveBeenCalledWith(
      "Voice input is unavailable while Anarlog is recording a meeting.",
    );
  });
});

type Composer = ReturnType<typeof renderComposer>;

function paragraphs(text: string) {
  return {
    type: "doc",
    content: text.split("\n").map((line) => ({
      type: "paragraph",
      ...(line ? { content: [{ type: "text", text: line }] } : {}),
    })),
  };
}

/** The real chat editor, with the caret at the end of `text`. */
function renderComposer(text: string) {
  const ref = createRef<ChatEditorHandle>();
  const mount = () => {
    const rendered = render(<ChatEditor ref={ref} />);
    act(() => ref.current!.replaceContent(paragraphs(text), "end"));
    return rendered;
  };
  let rendered = mount();
  const selectAll = () =>
    fireEvent.keyDown(rendered.container.querySelector('[role="textbox"]')!, {
      key: "a",
      ctrlKey: true,
    });
  return {
    ref,
    selectAll,
    /** The same content and selection, in a different editor. */
    remount: () => {
      rendered.unmount();
      rendered = mount();
      selectAll();
    },
    text: () =>
      (ref.current?.getJSON()?.content ?? [])
        .map((block) =>
          (block.content ?? []).map((node) => node.text ?? "").join(""),
        )
        .join("\n"),
  };
}

async function dictate(
  result: { current: ReturnType<typeof useDictation> },
  { settle = true } = {},
) {
  await act(async () => {
    await result.current.start();
  });
  await waitFor(() => expect(result.current.phase).toBe("recording"));
  await act(async () => {
    await result.current.stop();
  });
  if (settle) await waitFor(() => expect(result.current.phase).toBe("idle"));
  else await waitFor(() => expect(result.current.phase).toBe("transcribing"));
}

function pending<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((settle) => {
    resolve = settle;
  });
  return { promise, resolve };
}
