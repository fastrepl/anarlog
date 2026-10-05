import {
  cleanup,
  fireEvent,
  render,
  renderHook,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { EditorView } from "~/store/zustand/tabs/schema";

type CapturedMenuItem =
  | {
      id: string;
      text: string;
      action: () => void;
      disabled?: boolean;
    }
  | { separator: true };

const hoisted = vi.hoisted(() => ({
  enhance: vi.fn(),
  regenerateTranscript: vi.fn(),
  startListening: vi.fn(),
  stopListening: vi.fn(),
  stopTranscription: vi.fn(),
  requestMainListenerControl: vi.fn(),
  deleteRecording: vi.fn(),
  activeTemplateTitle: "Customer Call",
  audioExists: true,
  audioExistsResolved: true,
  hasTranscript: true,
  enhancedNotes: [{ id: "note-1" }],
  canShowTranscript: true,
  liveSegments: [] as unknown[],
  liveSessionId: null as string | null,
  liveAmplitude: { mic: 0.5, speaker: 0.25 },
  liveDegraded: null as unknown,
  liveMuted: false,
  sessionMode: "inactive",
  sessionEvent: null as { ended_at?: string } | null,
  transcriptMetadata: [] as Array<{
    id: string;
    sessionId: string;
    startedAt: number;
    endedAt?: number;
    hasWords: boolean;
  }>,
  nowMs: new Date("2026-06-05T10:31:00.000Z").getTime(),
  isMainWebviewWindow: true,
  isDeletingRecording: false,
  updateSession: vi.fn(() => Promise.resolve()),
  transcriptExportRequest: {},
  getSessionTranscriptRenderRequest: vi.fn(),
  transcriptSegments: [{ speaker: "Speaker 1", text: "Hello transcript" }],
  isGenerating: false,
  sessionTitle: "Weekly planning",
  nativeContextMenus: [] as CapturedMenuItem[][],
  userTemplates: [] as Array<{
    id: string;
    title: string;
    description: string;
    pinned: boolean;
    icon?: { type: "emoji"; value: string };
    sections: unknown[];
  }>,
}));

const lingui = vi.hoisted(() => {
  type LinguiDescriptor = {
    message?: string;
    values?: Record<string, unknown>;
  };
  const isDescriptor = (value: unknown): value is LinguiDescriptor =>
    Boolean(value) && typeof value === "object" && !Array.isArray(value);
  const t = (
    input: TemplateStringsArray | LinguiDescriptor | string,
    ...values: unknown[]
  ) => {
    if (typeof input === "string") {
      return input;
    }

    if (isDescriptor(input)) {
      let message = input.message ?? "";
      const replacements =
        input.values ??
        values.find(
          (value): value is Record<string, unknown> =>
            Boolean(value) &&
            typeof value === "object" &&
            !Array.isArray(value),
        );

      if (replacements) {
        for (const [key, value] of Object.entries(replacements)) {
          message = message.split(`{${key}}`).join(String(value));
        }
      }

      return message;
    }

    return Array.from(input).reduce(
      (text, part, index) => `${text}${part}${values[index] ?? ""}`,
      "",
    );
  };

  return { t };
});

vi.mock("@lingui/react/macro", () => ({
  useLingui: () => ({
    _: lingui.t,
    t: lingui.t,
  }),
}));

vi.mock("@lingui/react", () => ({
  useLingui: () => ({
    _: lingui.t,
    t: lingui.t,
  }),
}));

vi.mock("@anlg/editor/markdown", () => ({
  json2md: () => "",
  parseJsonContent: () => ({}),
}));

vi.mock("@anlg/plugin-analytics", () => ({
  commands: {
    event: vi.fn(),
  },
}));

vi.mock("@anlg/ui/components/ui/spinner", () => ({
  Spinner: () => <span data-testid="view-spinner" />,
}));

vi.mock("@anlg/ui/components/ui/dancing-sticks", () => ({
  DancingSticks: () => <span data-testid="dancing-sticks" />,
}));

vi.mock("~/audio-player", () => ({
  useAudioPlayer: () => ({
    audioExists: hoisted.audioExists,
    audioExistsResolved: hoisted.audioExistsResolved,
    deleteRecording: hoisted.deleteRecording,
    isDeletingRecording: hoisted.isDeletingRecording,
  }),
}));

vi.mock("~/calendar/hooks", () => ({
  useNow: () => new Date(hoisted.nowMs),
}));

vi.mock("~/ai/hooks", () => ({
  useAITaskTask: () => ({
    isIdle: true,
    isGenerating: hoisted.isGenerating,
    isError: false,
    error: null,
    start: vi.fn(),
    cancel: vi.fn(),
  }),
  useLanguageModel: () => "model",
  useLLMConnectionStatus: () => "connected",
  useTitleGenerating: () => false,
}));

vi.mock("~/session/enhance-config", () => ({
  shouldShowEmptySummaryConfigError: () => false,
}));

vi.mock("~/session/components/shared", () => ({
  useHasTranscript: () => hoisted.hasTranscript,
  useCanShowTranscript: () => hoisted.canShowTranscript,
}));

vi.mock("~/stt/queries", async (importOriginal) => ({
  ...(await importOriginal()),
  useSessionTranscriptMetadata: () => hoisted.transcriptMetadata,
}));

vi.mock("~/session/hooks/useSessionEvent", () => ({
  useSessionEvent: () => hoisted.sessionEvent,
}));

vi.mock("~/services/enhancer", () => ({
  getEnhancerService: () => ({ enhance: hoisted.enhance }),
}));

vi.mock("~/session/queries", () => ({
  deleteEnhancedNote: vi.fn(() => Promise.resolve()),
  useEnhancedNote: () => ({
    content: "",
    templateId: "template-1",
    title: "Summary",
  }),
  useEnhancedNoteRecords: () => hoisted.enhancedNotes,
  useFolderIcons: () => ({}),
  useFolderPaths: () => [],
  useSession: () => ({
    folder_id: "",
    raw_md: "",
    title: hoisted.sessionTitle,
  }),
  useUpdateSession: () => hoisted.updateSession,
}));

vi.mock("~/session/components/note-input/transcript/actions", () => ({
  useRegenerateTranscript: () => hoisted.regenerateTranscript,
}));

vi.mock("~/session/components/note-input/transcript/export-data", () => ({
  buildTranscriptExportSegments: () =>
    Promise.resolve(hoisted.transcriptSegments),
  formatTranscriptExportSegments: (
    segments: Array<{ speaker: string | null; text: string }>,
  ) =>
    segments
      .map((segment) => `${segment.speaker ?? "Speaker"}: ${segment.text}`)
      .join("\n\n"),
}));

vi.mock(
  "~/session/components/note-input/transcript/render-request-hooks",
  () => ({
    getSessionTranscriptRenderRequest:
      hoisted.getSessionTranscriptRenderRequest,
  }),
);

vi.mock("~/shared/hooks/useNativeContextMenu", () => ({
  useNativeContextMenu: (items: CapturedMenuItem[]) => {
    hoisted.nativeContextMenus.push(items);
    return vi.fn();
  },
}));

vi.mock("~/shared/ui/resource-list", () => ({
  useWebResources: () => ({ data: [], isLoading: false }),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: vi.fn((selector: (state: unknown) => unknown) =>
    selector({
      openNew: vi.fn(),
      select: vi.fn(),
      updateTemplatesTabState: vi.fn(),
    }),
  ),
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (
    selector: (state: {
      batch: Record<string, unknown>;
      live: {
        sessionId: string | null;
        finalizingBySession: Record<string, unknown>;
        amplitude: { mic: number; speaker: number };
        degraded: unknown;
        muted: boolean;
      };
      liveSegments: unknown[];
      getSessionMode: (sessionId?: string) => string;
      stop: () => void;
      stopTranscription: (sessionId: string) => void;
    }) => unknown,
  ) =>
    selector({
      batch: {},
      live: {
        sessionId: hoisted.liveSessionId,
        finalizingBySession: {},
        amplitude: hoisted.liveAmplitude,
        degraded: hoisted.liveDegraded,
        muted: hoisted.liveMuted,
      },
      liveSegments: hoisted.liveSegments,
      getSessionMode: () => hoisted.sessionMode,
      stop: hoisted.stopListening,
      stopTranscription: hoisted.stopTranscription,
    }),
}));

vi.mock("~/stt/useStartListeningWithBatchOverride", () => ({
  useStartListeningWithBatchOverride: () => hoisted.startListening,
}));

vi.mock("~/stt/window-control", () => ({
  isMainWebviewWindow: () => hoisted.isMainWebviewWindow,
  requestMainListenerControl: hoisted.requestMainListenerControl,
}));

vi.mock("~/templates", () => ({
  DEFAULT_TEMPLATE_ICON: {
    type: "icon",
    value: "notebook-tabs",
    color: "#9ca3af",
  },
  TemplateIconGlyph: ({ icon }: { icon?: { type: string; value: string } }) => (
    <span aria-hidden data-testid="template-icon">
      {icon?.value}
    </span>
  ),
  filterWebTemplatesAgainstUserTemplates: () => [],
  getTemplateCreatorLabel: () => "You",
  parseWebTemplates: () => [],
  useCreateTemplate: () => vi.fn(),
  useOpenTemplatesTab: () => vi.fn(),
  useTemplateCreatorName: () => "You",
  useUserTemplate: () => ({ data: { title: hoisted.activeTemplateTitle } }),
  useUserTemplates: () => hoisted.userTemplates,
}));

import { SessionViewSwitcher, useEditorTabs } from "./header";

let clipboardDescriptor: PropertyDescriptor | undefined;

const ALL_TABS: EditorView[] = [
  { type: "enhanced", id: "note-1" },
  { type: "raw" },
  { type: "transcript" },
];

function renderSwitcher(
  props: Partial<React.ComponentProps<typeof SessionViewSwitcher>> = {},
) {
  const handleTabChange = vi.fn();
  const view = render(
    <SessionViewSwitcher
      sessionId="session-1"
      editorTabs={ALL_TABS}
      currentTab={{ type: "raw" }}
      handleTabChange={handleTabChange}
      {...props}
    />,
  );
  return { ...view, handleTabChange };
}

function transcriptMenu() {
  const menu = [...hoisted.nativeContextMenus]
    .reverse()
    .find((items) =>
      items.some(
        (item) => "id" in item && item.id === "copy-transcript-session-1",
      ),
    );
  if (!menu) {
    throw new Error("Transcript context menu not found");
  }
  return menu.filter(
    (item): item is Extract<CapturedMenuItem, { id: string }> => "id" in item,
  );
}

describe("SessionViewSwitcher", () => {
  beforeEach(() => {
    clipboardDescriptor = Object.getOwnPropertyDescriptor(
      navigator,
      "clipboard",
    );
    hoisted.enhance.mockReset();
    hoisted.regenerateTranscript.mockReset();
    hoisted.startListening.mockReset();
    hoisted.stopListening.mockReset();
    hoisted.stopTranscription.mockReset();
    hoisted.requestMainListenerControl.mockReset();
    hoisted.deleteRecording.mockReset();
    hoisted.activeTemplateTitle = "Customer Call";
    hoisted.audioExists = true;
    hoisted.audioExistsResolved = true;
    hoisted.hasTranscript = true;
    hoisted.canShowTranscript = true;
    hoisted.liveSegments = [];
    hoisted.liveSessionId = null;
    hoisted.liveAmplitude = { mic: 0.5, speaker: 0.25 };
    hoisted.liveDegraded = null;
    hoisted.liveMuted = false;
    hoisted.sessionMode = "inactive";
    hoisted.sessionEvent = null;
    hoisted.transcriptMetadata = [
      {
        id: "transcript-1",
        sessionId: "session-1",
        startedAt: 0,
        hasWords: true,
      },
    ];
    hoisted.nowMs = new Date("2026-06-05T10:31:00.000Z").getTime();
    hoisted.isMainWebviewWindow = true;
    hoisted.isDeletingRecording = false;
    hoisted.transcriptExportRequest = {};
    hoisted.getSessionTranscriptRenderRequest.mockReset();
    hoisted.getSessionTranscriptRenderRequest.mockImplementation(() =>
      Promise.resolve(hoisted.transcriptExportRequest),
    );
    hoisted.transcriptSegments = [
      { speaker: "Speaker 1", text: "Hello transcript" },
    ];
    hoisted.isGenerating = false;
    hoisted.sessionTitle = "Weekly planning";
    hoisted.nativeContextMenus = [];
    hoisted.userTemplates = [];
  });

  afterEach(() => {
    cleanup();
    if (clipboardDescriptor) {
      Object.defineProperty(navigator, "clipboard", clipboardDescriptor);
    } else {
      Reflect.deleteProperty(navigator, "clipboard");
    }
  });

  it("navigates between views and opens the template picker from the active summary", () => {
    const { handleTabChange, rerender } = renderSwitcher();

    fireEvent.click(screen.getByRole("button", { name: "Customer Call" }));
    fireEvent.click(screen.getByRole("button", { name: "Transcript" }));

    expect(handleTabChange).toHaveBeenNthCalledWith(1, {
      type: "enhanced",
      id: "note-1",
    });
    expect(handleTabChange).toHaveBeenNthCalledWith(2, { type: "transcript" });

    rerender(
      <SessionViewSwitcher
        sessionId="session-1"
        editorTabs={ALL_TABS}
        currentTab={{ type: "enhanced", id: "note-1" }}
        handleTabChange={handleTabChange}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Customer Call" }));

    expect(screen.getByPlaceholderText("Search templates...")).not.toBeNull();
  });

  it("hides the view switcher when the memo is the only view", () => {
    renderSwitcher({ editorTabs: [{ type: "raw" }] });

    expect(
      screen.queryByRole("group", { name: "Session note views" }),
    ).toBeNull();
  });

  it.each([
    [
      "Decision Log",
      /Decision Log/,
      { templateId: "template-2", templateTitle: "Decision Log" },
    ],
    ["Auto", "Auto", { templateId: null, templateTitle: undefined }],
  ])(
    "regenerates the current summary in place with the %s template",
    async (_label, option, expected) => {
      hoisted.userTemplates = [
        {
          id: "template-2",
          title: "Decision Log",
          description: "",
          pinned: false,
          sections: [],
        },
      ];
      hoisted.enhance.mockResolvedValue({ type: "started", noteId: "note-1" });
      const { handleTabChange } = renderSwitcher({
        editorTabs: [{ type: "enhanced", id: "note-1" }, { type: "raw" }],
        currentTab: { type: "enhanced", id: "note-1" },
      });

      fireEvent.click(screen.getByRole("button", { name: "Customer Call" }));
      fireEvent.click(screen.getByRole("button", { name: option }));

      expect(hoisted.enhance).toHaveBeenCalledWith("session-1", {
        ...expected,
        targetNoteId: "note-1",
      });
      await waitFor(() =>
        expect(handleTabChange).toHaveBeenCalledWith({
          type: "enhanced",
          id: "note-1",
        }),
      );
    },
  );

  it("builds transcript copy data only when the Copy action runs", async () => {
    const writeText = vi.fn(() => Promise.resolve());
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });

    renderSwitcher({ currentTab: { type: "transcript" } });

    expect(hoisted.getSessionTranscriptRenderRequest).not.toHaveBeenCalled();

    transcriptMenu()
      .find((item) => item.text === "Copy")
      ?.action();

    await waitFor(() =>
      expect(hoisted.getSessionTranscriptRenderRequest).toHaveBeenCalledTimes(
        1,
      ),
    );
    expect(hoisted.getSessionTranscriptRenderRequest).toHaveBeenCalledWith(
      "session-1",
    );
    await waitFor(() =>
      expect(writeText).toHaveBeenCalledWith("Speaker 1: Hello transcript"),
    );
  });

  it.each([
    [
      "inactive with audio",
      { mode: "inactive", audio: true, resolved: true },
      ["Copy", "Resume listening", "Re-transcribe", "Delete recording"],
    ],
    [
      "inactive without audio",
      { mode: "inactive", audio: false, resolved: true },
      ["Copy", "Resume listening"],
    ],
    [
      "audio lookup pending",
      { mode: "inactive", audio: true, resolved: false },
      ["Copy", "Resume listening", "Delete recording"],
    ],
    [
      "batch processing",
      { mode: "running_batch", audio: false, resolved: true },
      ["Copy", "Resume listening"],
    ],
    [
      "finalizing",
      { mode: "finalizing", audio: false, resolved: true },
      ["Copy"],
    ],
  ])(
    "offers transcript menu actions when %s",
    (_label, { mode, audio, resolved }, expected) => {
      hoisted.sessionMode = mode;
      hoisted.audioExists = audio;
      hoisted.audioExistsResolved = resolved;
      renderSwitcher({ currentTab: { type: "transcript" } });

      expect(transcriptMenu().map((item) => item.text)).toEqual(expected);
    },
  );

  it("disables Copy when the session has no transcript", () => {
    hoisted.hasTranscript = false;
    hoisted.transcriptMetadata = [
      {
        id: "transcript-1",
        sessionId: "session-1",
        startedAt: 0,
        hasWords: false,
      },
    ];
    renderSwitcher({ currentTab: { type: "transcript" } });

    expect(
      transcriptMenu().find((item) => item.text === "Copy")?.disabled,
    ).toBe(true);
  });

  it("enables Copy when transcript words are still pending compaction", () => {
    hoisted.hasTranscript = false;
    hoisted.transcriptMetadata = [
      {
        id: "transcript-1",
        sessionId: "session-1",
        startedAt: 0,
        hasWords: true,
      },
    ];
    renderSwitcher({ currentTab: { type: "transcript" } });

    expect(
      transcriptMenu().find((item) => item.text === "Copy")?.disabled,
    ).toBe(false);
  });

  it.each([
    ["main window", true],
    ["standalone window", false],
  ])("resumes listening from the transcript menu in the %s", (_label, main) => {
    hoisted.isMainWebviewWindow = main;
    renderSwitcher({ currentTab: { type: "transcript" } });

    transcriptMenu()
      .find((item) => item.id === "resume-listening-session-1")
      ?.action();

    if (main) {
      expect(hoisted.startListening).toHaveBeenCalledTimes(1);
      expect(hoisted.requestMainListenerControl).not.toHaveBeenCalled();
    } else {
      expect(hoisted.requestMainListenerControl).toHaveBeenCalledWith(
        "start",
        "session-1",
      );
      expect(hoisted.startListening).not.toHaveBeenCalled();
    }
  });

  it("toggles transcript editing from the selected transcript tab", () => {
    const onTranscriptEditModeChange = vi.fn();
    const { handleTabChange, rerender } = renderSwitcher({
      currentTab: { type: "transcript" },
      onTranscriptEditModeChange,
    });

    fireEvent.click(screen.getByRole("button", { name: "Transcript" }));
    expect(onTranscriptEditModeChange).toHaveBeenLastCalledWith(true);

    rerender(
      <SessionViewSwitcher
        sessionId="session-1"
        editorTabs={ALL_TABS}
        currentTab={{ type: "transcript" }}
        handleTabChange={handleTabChange}
        transcriptEditMode
        onTranscriptEditModeChange={onTranscriptEditModeChange}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Transcript" }));

    expect(onTranscriptEditModeChange).toHaveBeenLastCalledWith(false);
    expect(handleTabChange).not.toHaveBeenCalled();
  });

  it.each([
    ["listening on the memo tab", "active", { type: "raw" }, false],
    [
      "listening on the transcript tab",
      "active",
      { type: "transcript" },
      false,
    ],
    ["finalizing", "finalizing", { type: "transcript" }, true],
    ["batch transcribing", "running_batch", { type: "transcript" }, true],
    ["inactive on the memo tab", "inactive", { type: "raw" }, false],
  ] as const)(
    "only navigates from the transcript tab while %s",
    (_label, mode, currentTab, isTranscribing) => {
      hoisted.sessionMode = mode;
      const onTranscriptEditModeChange = vi.fn();
      const { handleTabChange } = renderSwitcher({
        currentTab,
        isTranscribing,
        onTranscriptEditModeChange,
      });

      fireEvent.click(screen.getByRole("button", { name: "Transcript" }));

      expect(handleTabChange).toHaveBeenCalledWith({ type: "transcript" });
      expect(onTranscriptEditModeChange).not.toHaveBeenCalled();
      expect(hoisted.stopListening).not.toHaveBeenCalled();
      expect(hoisted.stopTranscription).not.toHaveBeenCalled();
      expect(hoisted.startListening).not.toHaveBeenCalled();
      expect(hoisted.requestMainListenerControl).not.toHaveBeenCalled();
    },
  );
});

describe("useEditorTabs", () => {
  it.each([
    [true, ALL_TABS],
    [false, [{ type: "enhanced", id: "note-1" }, { type: "raw" }]],
  ])(
    "includes the transcript tab only when it can be shown (%s)",
    (canShowTranscript, expected) => {
      hoisted.canShowTranscript = canShowTranscript;
      hoisted.hasTranscript = true;
      hoisted.sessionMode = "inactive";
      hoisted.enhancedNotes = [];

      const { result, rerender } = renderHook(() =>
        useEditorTabs({ sessionId: "session-1" }),
      );

      expect(result.current).toEqual([
        { type: "enhanced", id: "summary:session-1" },
        { type: "raw" },
        ...(canShowTranscript ? [{ type: "transcript" }] : []),
      ]);
      hoisted.enhancedNotes = [{ id: "note-1" }];
      rerender();
      expect(result.current).toEqual(expected);
    },
  );
});
