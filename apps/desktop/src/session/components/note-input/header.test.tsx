import {
  cleanup,
  fireEvent,
  render,
  renderHook,
  screen,
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
  hasTranscript: true,
  canShowInsights: false,
  liveSegments: [] as unknown[],
  liveSessionId: null as string | null,
  liveAmplitude: { mic: 0.5, speaker: 0.25 },
  liveDegraded: null as unknown,
  liveMuted: false,
  sessionMode: "inactive",
  isMainWebviewWindow: true,
  isDeletingRecording: false,
  transcriptExportRequest: {},
  transcriptRenderDataCalls: 0,
  transcriptSegments: [{ speaker: "Speaker 1", text: "Hello transcript" }],
  isGenerating: false,
  nativeContextMenus: [] as CapturedMenuItem[][],
  userTemplates: [] as Array<{
    id: string;
    title: string;
    description: string;
    pinned: boolean;
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

vi.mock("@hypr/editor/markdown", () => ({
  json2md: () => "",
  parseJsonContent: () => ({}),
}));

vi.mock("@hypr/plugin-analytics", () => ({
  commands: {
    event: vi.fn(),
  },
}));

vi.mock("@hypr/ui/components/ui/spinner", () => ({
  Spinner: () => <span data-testid="tab-spinner" />,
}));

vi.mock("@hypr/ui/components/ui/dancing-sticks", () => ({
  DancingSticks: () => <span data-testid="dancing-sticks" />,
}));

vi.mock("~/audio-player", () => ({
  useAudioPlayer: () => ({
    audioExists: hoisted.audioExists,
    deleteRecording: hoisted.deleteRecording,
    isDeletingRecording: hoisted.isDeletingRecording,
  }),
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
}));

vi.mock("~/session/enhance-config", () => ({
  shouldShowEmptySummaryConfigError: () => false,
}));

vi.mock("~/session/components/shared", () => ({
  useHasTranscript: () => hoisted.hasTranscript,
  useCanShowTranscript: (
    sessionId: string,
    { audioExists = false }: { audioExists?: boolean } = {},
  ) =>
    hoisted.hasTranscript ||
    (audioExists &&
      hoisted.sessionMode !== "active" &&
      hoisted.sessionMode !== "finalizing") ||
    hoisted.sessionMode === "active" ||
    hoisted.sessionMode === "finalizing" ||
    (hoisted.liveSessionId === sessionId && hoisted.liveSegments.length > 0) ||
    hoisted.sessionMode === "running_batch",
}));

vi.mock("~/session/insights/past-notes", () => ({
  useCanShowInsights: () => hoisted.canShowInsights,
}));

vi.mock("~/session/hooks/useEnhancedNotes", () => ({
  useEnsureDefaultSummary: vi.fn(),
}));

vi.mock("~/services/enhancer", () => ({
  getEnhancerService: () => ({ enhance: hoisted.enhance }),
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
    useSessionTranscriptRenderData: () => {
      hoisted.transcriptRenderDataCalls += 1;

      return {
        request: hoisted.transcriptExportRequest,
        transcriptRows: [],
      };
    },
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

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  INDEXES: {
    enhancedNotesBySession: "enhancedNotesBySession",
  },
  UI: {
    useCell: (table: string, _row: string, cell: string) => {
      if (table === "enhanced_notes" && cell === "title") {
        return "Summary";
      }

      if (table === "enhanced_notes" && cell === "content") {
        return "";
      }

      if (table === "enhanced_notes" && cell === "template_id") {
        return "template-1";
      }

      if (table === "sessions" && cell === "raw_md") {
        return "";
      }

      return undefined;
    },
    useSliceRowIds: () => ["note-1"],
    useStore: () => ({
      delRow: vi.fn(),
      setPartialRow: vi.fn(),
    }),
  },
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

vi.mock("~/stt/useStartListening", () => ({
  useStartListening: () => hoisted.startListening,
}));

vi.mock("~/stt/window-control", () => ({
  isMainWebviewWindow: () => hoisted.isMainWebviewWindow,
  requestMainListenerControl: hoisted.requestMainListenerControl,
}));

vi.mock("~/templates", () => ({
  filterWebTemplatesAgainstUserTemplates: () => [],
  getTemplateCreatorLabel: () => "You",
  parseWebTemplates: () => [],
  useCreateTemplate: () => vi.fn(),
  useTemplateCreatorName: () => "You",
  useUserTemplate: () => ({ data: { title: hoisted.activeTemplateTitle } }),
  useUserTemplates: () => hoisted.userTemplates,
}));

import { Header, useEditorTabs } from "./header";

describe("Header", () => {
  beforeEach(() => {
    hoisted.enhance.mockReset();
    hoisted.regenerateTranscript.mockReset();
    hoisted.startListening.mockReset();
    hoisted.stopListening.mockReset();
    hoisted.stopTranscription.mockReset();
    hoisted.requestMainListenerControl.mockReset();
    hoisted.deleteRecording.mockReset();
    hoisted.activeTemplateTitle = "Customer Call";
    hoisted.audioExists = true;
    hoisted.hasTranscript = true;
    hoisted.canShowInsights = false;
    hoisted.liveSegments = [];
    hoisted.liveSessionId = null;
    hoisted.liveAmplitude = { mic: 0.5, speaker: 0.25 };
    hoisted.liveDegraded = null;
    hoisted.liveMuted = false;
    hoisted.sessionMode = "inactive";
    hoisted.isMainWebviewWindow = true;
    hoisted.isDeletingRecording = false;
    hoisted.transcriptExportRequest = {};
    hoisted.transcriptRenderDataCalls = 0;
    hoisted.transcriptSegments = [
      { speaker: "Speaker 1", text: "Hello transcript" },
    ];
    hoisted.isGenerating = false;
    hoisted.nativeContextMenus = [];
    hoisted.userTemplates = [];
  });

  afterEach(() => {
    cleanup();
  });

  it("renders icon tabs and focuses summary before opening the template picker", () => {
    const editorTabs: EditorView[] = [
      { type: "enhanced", id: "note-1" },
      { type: "raw" },
      { type: "transcript" },
    ];
    const handleTabChange = vi.fn();

    const view = render(
      <Header
        sessionId="session-1"
        editorTabs={editorTabs}
        currentTab={{ type: "raw" }}
        handleTabChange={handleTabChange}
      />,
    );

    const summaryTab = screen.getByRole("button", { name: "Customer Call" });
    const memoTab = screen.getByRole("button", { name: "Memos" });
    const transcriptTab = screen.getByRole("button", { name: "Transcript" });
    const tabList = screen.getByRole("tablist");

    expect(summaryTab.getAttribute("data-state")).toBeNull();
    expect(tabList.getAttribute("data-tauri-drag-region")).toBe("false");
    expect(tabList.className).toContain("h-[30px]");
    expect(tabList.className).toContain("p-[2px]");
    expect(tabList.className).toContain("gap-[2px]");
    expect(tabList.className).toContain("bg-foreground/10");
    expect(tabList.className).toContain("dark:bg-accent/55");
    expect(summaryTab.getAttribute("aria-current")).toBeNull();
    expect(memoTab.getAttribute("aria-current")).toBe("page");
    expect(memoTab.textContent).toBe("Memos");
    expect(memoTab.className).toContain("h-[26px]");
    expect(memoTab.className).not.toContain("-my-px");
    expect(memoTab.className).toContain("bg-white");
    expect(memoTab.className).toContain("text-foreground");
    expect(memoTab.className).toContain("shadow-xs");
    expect(memoTab.className).toContain("dark:text-foreground");
    expect(memoTab.className).toContain("dark:bg-accent");
    expect(memoTab.className).toContain("dark:shadow-none");
    expect(summaryTab.className).toContain("h-[26px]");
    expect(summaryTab.className).toContain("dark:hover:bg-accent/80");
    expect(summaryTab.querySelector("svg")).not.toBeNull();
    expect(summaryTab.querySelectorAll("svg")).toHaveLength(1);
    expect(transcriptTab.querySelector("svg")).not.toBeNull();
    expect(summaryTab.textContent).toBe("");
    expect(transcriptTab.textContent).toBe("");
    expect(summaryTab.getAttribute("title")).toBe(
      "Customer Call was used to generate this summary.",
    );

    fireEvent.click(summaryTab);

    expect(handleTabChange).toHaveBeenNthCalledWith(1, {
      type: "enhanced",
      id: "note-1",
    });

    view.rerender(
      <Header
        sessionId="session-1"
        editorTabs={editorTabs}
        currentTab={{ type: "enhanced", id: "note-1" }}
        handleTabChange={handleTabChange}
      />,
    );

    const activeSummaryTab = screen.getByRole("button", {
      name: "Customer Call",
    });
    expect(activeSummaryTab.textContent).toBe("Customer Call");
    expect(activeSummaryTab.className).toContain("text-foreground");
    expect(activeSummaryTab.className).toContain("dark:text-foreground");
    expect(activeSummaryTab.className).toContain("dark:bg-accent");
    expect(activeSummaryTab.querySelectorAll("svg")).toHaveLength(2);

    fireEvent.click(activeSummaryTab);

    expect(screen.getByPlaceholderText("Search templates...")).not.toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Memos" }));

    view.rerender(
      <Header
        sessionId="session-1"
        editorTabs={editorTabs}
        currentTab={{ type: "raw" }}
        handleTabChange={handleTabChange}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Customer Call" }));

    expect(handleTabChange).toHaveBeenNthCalledWith(2, { type: "raw" });
    expect(handleTabChange).toHaveBeenNthCalledWith(3, {
      type: "enhanced",
      id: "note-1",
    });
  });

  it("renders a raw-only memo tab without the tab tray", () => {
    render(
      <Header
        sessionId="session-1"
        editorTabs={[{ type: "raw" }]}
        currentTab={{ type: "raw" }}
        handleTabChange={vi.fn()}
      />,
    );

    const memoTab = screen.getByRole("button", { name: "Memos" });
    const tabList = screen.getByRole("tablist");

    expect(tabList.className).not.toContain("h-[30px]");
    expect(tabList.className).not.toContain("bg-foreground/10");
    expect(tabList.className).not.toContain("rounded-full");
    expect(memoTab.textContent).toBe("Memos");
    expect(memoTab.className).toContain("h-7");
    expect(memoTab.className).toContain("bg-white");
    expect(memoTab.className).toContain("border");
    expect(memoTab.className).not.toContain("bg-foreground/10");
  });

  it("can switch from transcript back to memo or summary tabs", () => {
    const editorTabs: EditorView[] = [
      { type: "enhanced", id: "note-1" },
      { type: "raw" },
      { type: "transcript" },
    ];
    const handleTabChange = vi.fn();

    render(
      <Header
        sessionId="session-1"
        editorTabs={editorTabs}
        currentTab={{ type: "transcript" }}
        handleTabChange={handleTabChange}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Memos" }));
    expect(screen.getByRole("button", { name: "Transcript" }).textContent).toBe(
      "Transcript",
    );

    fireEvent.click(screen.getByRole("button", { name: "Customer Call" }));

    expect(handleTabChange).toHaveBeenNthCalledWith(1, { type: "raw" });
    expect(handleTabChange).toHaveBeenNthCalledWith(2, {
      type: "enhanced",
      id: "note-1",
    });
  });

  it("adds recording actions to the transcript tab context menu", () => {
    const editorTabs: EditorView[] = [
      { type: "enhanced", id: "note-1" },
      { type: "raw" },
      { type: "transcript" },
    ];

    render(
      <Header
        sessionId="session-1"
        editorTabs={editorTabs}
        currentTab={{ type: "transcript" }}
        handleTabChange={vi.fn()}
      />,
    );

    const menu = findContextMenu("copy-transcript-session-1");

    expect(
      menu.map((item) => ("text" in item ? item.text : "separator")),
    ).toEqual(["Copy", "Regenerate", "Delete recording"]);
    expect(menu.find(isMenuItem)?.disabled).toBe(false);
    expect(
      menu.find(
        (item): item is Extract<CapturedMenuItem, { id: string }> =>
          "id" in item && item.id === "delete-recording-session-1",
      )?.disabled,
    ).toBe(false);
  });

  it("does not prepare transcript export data while the transcript tab is inactive", () => {
    const editorTabs: EditorView[] = [
      { type: "enhanced", id: "note-1" },
      { type: "raw" },
      { type: "transcript" },
    ];

    const view = render(
      <Header
        sessionId="session-1"
        editorTabs={editorTabs}
        currentTab={{ type: "raw" }}
        handleTabChange={vi.fn()}
      />,
    );

    expect(hoisted.transcriptRenderDataCalls).toBe(0);

    view.rerender(
      <Header
        sessionId="session-1"
        editorTabs={editorTabs}
        currentTab={{ type: "transcript" }}
        handleTabChange={vi.fn()}
      />,
    );

    expect(hoisted.transcriptRenderDataCalls).toBe(1);
  });

  it("omits transcript recording actions when recording is missing", () => {
    hoisted.audioExists = false;
    const editorTabs: EditorView[] = [
      { type: "enhanced", id: "note-1" },
      { type: "raw" },
      { type: "transcript" },
    ];

    render(
      <Header
        sessionId="session-1"
        editorTabs={editorTabs}
        currentTab={{ type: "transcript" }}
        handleTabChange={vi.fn()}
      />,
    );

    const menu = findContextMenu("copy-transcript-session-1");

    expect(
      menu.map((item) => ("text" in item ? item.text : "separator")),
    ).toEqual(["Copy"]);
  });

  it("replaces the current enhanced note when changing templates", () => {
    hoisted.userTemplates = [
      {
        id: "template-2",
        title: "Decision Log",
        description: "",
        pinned: false,
        sections: [],
      },
    ];
    hoisted.enhance.mockReturnValue({
      type: "started",
      noteId: "note-1",
    });
    const editorTabs: EditorView[] = [
      { type: "enhanced", id: "note-1" },
      { type: "raw" },
    ];
    const handleTabChange = vi.fn();

    render(
      <Header
        sessionId="session-1"
        editorTabs={editorTabs}
        currentTab={{ type: "enhanced", id: "note-1" }}
        handleTabChange={handleTabChange}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Customer Call" }));
    fireEvent.click(screen.getByRole("button", { name: /Decision Log/ }));

    expect(hoisted.enhance).toHaveBeenCalledWith("session-1", {
      templateId: "template-2",
      targetNoteId: "note-1",
      templateTitle: "Decision Log",
    });
    expect(handleTabChange).toHaveBeenCalledWith({
      type: "enhanced",
      id: "note-1",
    });
  });

  it("replaces the current enhanced note with auto generation", () => {
    hoisted.userTemplates = [
      {
        id: "template-2",
        title: "Decision Log",
        description: "",
        pinned: false,
        sections: [],
      },
    ];
    hoisted.enhance.mockReturnValue({
      type: "started",
      noteId: "note-1",
    });
    const editorTabs: EditorView[] = [
      { type: "enhanced", id: "note-1" },
      { type: "raw" },
    ];

    render(
      <Header
        sessionId="session-1"
        editorTabs={editorTabs}
        currentTab={{ type: "enhanced", id: "note-1" }}
        handleTabChange={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Customer Call" }));
    fireEvent.click(screen.getByRole("button", { name: "Auto" }));

    expect(hoisted.enhance).toHaveBeenCalledWith("session-1", {
      templateId: null,
      targetNoteId: "note-1",
      templateTitle: undefined,
    });
  });

  it("shows a spinner in the active enhanced tab while generating", () => {
    hoisted.isGenerating = true;
    const editorTabs: EditorView[] = [
      { type: "enhanced", id: "note-1" },
      { type: "raw" },
    ];

    render(
      <Header
        sessionId="session-1"
        editorTabs={editorTabs}
        currentTab={{ type: "enhanced", id: "note-1" }}
        handleTabChange={vi.fn()}
      />,
    );

    expect(screen.getByTestId("tab-spinner")).not.toBeNull();
    expect(
      screen.getByRole("button", { name: "Customer Call" }).textContent,
    ).toBe("Customer Call");
  });

  it("shows a spinner in the transcript tab while transcribing", () => {
    const editorTabs: EditorView[] = [
      { type: "enhanced", id: "note-1" },
      { type: "raw" },
      { type: "transcript" },
    ];

    render(
      <Header
        sessionId="session-1"
        editorTabs={editorTabs}
        currentTab={{ type: "raw" }}
        handleTabChange={vi.fn()}
        isTranscribing
      />,
    );

    const transcriptTab = screen.getByRole("button", { name: "Transcript" });

    expect(
      transcriptTab.querySelector("[data-testid='tab-spinner']"),
    ).not.toBeNull();
    expect(transcriptTab.querySelector("svg")).toBeNull();
  });

  it("stops transcription from the active transcript tab spinner", () => {
    const editorTabs: EditorView[] = [
      { type: "enhanced", id: "note-1" },
      { type: "raw" },
      { type: "transcript" },
    ];

    render(
      <Header
        sessionId="session-1"
        editorTabs={editorTabs}
        currentTab={{ type: "transcript" }}
        handleTabChange={vi.fn()}
        isTranscribing
        canStopTranscription
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Transcript" }));

    expect(hoisted.stopTranscription).toHaveBeenCalledWith("session-1");
  });

  it("resumes listening from the active transcript tab in the main window", () => {
    const handleTabChange = vi.fn();
    const editorTabs: EditorView[] = [
      { type: "enhanced", id: "note-1" },
      { type: "raw" },
      { type: "transcript" },
    ];

    render(
      <Header
        sessionId="session-1"
        editorTabs={editorTabs}
        currentTab={{ type: "transcript" }}
        handleTabChange={handleTabChange}
      />,
    );

    const transcriptTab = screen.getByRole("button", { name: "Transcript" });

    expect(transcriptTab.className).toContain("w-[98px]");
    expect(transcriptTab.getAttribute("title")).toBe("Resume listening");
    expect(transcriptTab.getAttribute("data-hover-label")).toBe("Resume");
    expect(transcriptTab.textContent).toBe("Transcript");
    expect(transcriptTab.querySelector(".animate-ping")).not.toBeNull();

    fireEvent.click(transcriptTab);

    expect(hoisted.startListening).toHaveBeenCalledTimes(1);
    expect(hoisted.requestMainListenerControl).not.toHaveBeenCalled();
    expect(handleTabChange).not.toHaveBeenCalled();
  });

  it("delegates resume listening from the active transcript tab in standalone windows", () => {
    hoisted.isMainWebviewWindow = false;
    const editorTabs: EditorView[] = [
      { type: "enhanced", id: "note-1" },
      { type: "raw" },
      { type: "transcript" },
    ];

    render(
      <Header
        sessionId="session-1"
        editorTabs={editorTabs}
        currentTab={{ type: "transcript" }}
        handleTabChange={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Transcript" }));

    expect(hoisted.requestMainListenerControl).toHaveBeenCalledWith(
      "start",
      "session-1",
    );
    expect(hoisted.startListening).not.toHaveBeenCalled();
  });

  it("keeps inactive transcript tabs as navigation instead of resume actions", () => {
    const handleTabChange = vi.fn();
    const editorTabs: EditorView[] = [
      { type: "enhanced", id: "note-1" },
      { type: "raw" },
      { type: "transcript" },
    ];

    render(
      <Header
        sessionId="session-1"
        editorTabs={editorTabs}
        currentTab={{ type: "raw" }}
        handleTabChange={handleTabChange}
      />,
    );

    const transcriptTab = screen.getByRole("button", { name: "Transcript" });

    expect(transcriptTab.getAttribute("title")).toBeNull();

    fireEvent.click(transcriptTab);

    expect(handleTabChange).toHaveBeenCalledWith({ type: "transcript" });
    expect(hoisted.startListening).not.toHaveBeenCalled();
    expect(hoisted.requestMainListenerControl).not.toHaveBeenCalled();
  });

  it("shows live listening state on the inactive transcript tab without stopping on click", () => {
    hoisted.sessionMode = "active";
    const handleTabChange = vi.fn();
    const editorTabs: EditorView[] = [
      { type: "enhanced", id: "note-1" },
      { type: "raw" },
      { type: "transcript" },
    ];

    render(
      <Header
        sessionId="session-1"
        editorTabs={editorTabs}
        currentTab={{ type: "raw" }}
        handleTabChange={handleTabChange}
      />,
    );

    const transcriptTab = screen.getByRole("button", { name: "Transcript" });

    expect(screen.getByTestId("dancing-sticks")).not.toBeNull();
    expect(transcriptTab.className).toContain("bg-red-50");
    expect(transcriptTab.getAttribute("title")).toBeNull();

    fireEvent.click(transcriptTab);

    expect(handleTabChange).toHaveBeenCalledWith({ type: "transcript" });
    expect(hoisted.stopListening).not.toHaveBeenCalled();
    expect(hoisted.requestMainListenerControl).not.toHaveBeenCalled();
  });

  it("stops a live meeting from the active transcript tab in the main window", () => {
    hoisted.sessionMode = "active";
    const handleTabChange = vi.fn();
    const editorTabs: EditorView[] = [
      { type: "enhanced", id: "note-1" },
      { type: "raw" },
      { type: "transcript" },
    ];

    render(
      <Header
        sessionId="session-1"
        editorTabs={editorTabs}
        currentTab={{ type: "transcript" }}
        handleTabChange={handleTabChange}
      />,
    );

    const transcriptTab = screen.getByRole("button", { name: "Transcript" });

    expect(screen.getByTestId("dancing-sticks")).not.toBeNull();
    expect(transcriptTab.className).toContain("bg-red-50");
    expect(transcriptTab.getAttribute("title")).toBe("Stop listening");
    expect(transcriptTab.getAttribute("data-hover-label")).toBe("Stop");

    fireEvent.click(transcriptTab);

    expect(hoisted.stopListening).toHaveBeenCalledTimes(1);
    expect(hoisted.requestMainListenerControl).not.toHaveBeenCalled();
    expect(handleTabChange).not.toHaveBeenCalled();
  });

  it("delegates live meeting stop from the active transcript tab in standalone windows", () => {
    hoisted.sessionMode = "active";
    hoisted.isMainWebviewWindow = false;
    const editorTabs: EditorView[] = [
      { type: "enhanced", id: "note-1" },
      { type: "raw" },
      { type: "transcript" },
    ];

    render(
      <Header
        sessionId="session-1"
        editorTabs={editorTabs}
        currentTab={{ type: "transcript" }}
        handleTabChange={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Transcript" }));

    expect(hoisted.requestMainListenerControl).toHaveBeenCalledWith(
      "stop",
      "session-1",
    );
    expect(hoisted.stopListening).not.toHaveBeenCalled();
  });

  it("does not stop a finalizing live meeting from the transcript tab", () => {
    hoisted.sessionMode = "finalizing";
    const handleTabChange = vi.fn();
    const editorTabs: EditorView[] = [
      { type: "enhanced", id: "note-1" },
      { type: "raw" },
      { type: "transcript" },
    ];

    render(
      <Header
        sessionId="session-1"
        editorTabs={editorTabs}
        currentTab={{ type: "transcript" }}
        handleTabChange={handleTabChange}
      />,
    );

    const transcriptTab = screen.getByRole("button", { name: "Transcript" });

    expect(screen.getByTestId("dancing-sticks")).not.toBeNull();
    expect(transcriptTab.getAttribute("title")).toBeNull();

    fireEvent.click(transcriptTab);

    expect(hoisted.stopListening).not.toHaveBeenCalled();
    expect(hoisted.requestMainListenerControl).not.toHaveBeenCalled();
    expect(handleTabChange).toHaveBeenCalledWith({ type: "transcript" });
  });

  it("does not stop transcription from the active transcript tab while finalizing", () => {
    const handleTabChange = vi.fn();
    const editorTabs: EditorView[] = [
      { type: "enhanced", id: "note-1" },
      { type: "raw" },
      { type: "transcript" },
    ];

    render(
      <Header
        sessionId="session-1"
        editorTabs={editorTabs}
        currentTab={{ type: "transcript" }}
        handleTabChange={handleTabChange}
        isTranscribing
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Transcript" }));

    expect(hoisted.stopTranscription).not.toHaveBeenCalled();
    expect(handleTabChange).toHaveBeenCalledWith({ type: "transcript" });
  });

  it("includes the transcript tab when saved audio exists without transcript rows", () => {
    hoisted.hasTranscript = false;

    const { result } = renderHook(() =>
      useEditorTabs({ sessionId: "session-1", audioExists: true }),
    );

    expect(result.current).toEqual([
      { type: "enhanced", id: "note-1" },
      { type: "raw" },
      { type: "transcript" },
    ]);
  });

  it("includes the insights tab when past-note insights exist", () => {
    hoisted.canShowInsights = true;

    const { result } = renderHook(() =>
      useEditorTabs({ sessionId: "session-1", audioExists: true }),
    );

    expect(result.current).toEqual([
      { type: "enhanced", id: "note-1" },
      { type: "insights" },
      { type: "raw" },
      { type: "transcript" },
    ]);
  });

  it("includes the transcript tab for active meetings before transcript evidence arrives", () => {
    hoisted.hasTranscript = false;
    hoisted.sessionMode = "active";
    hoisted.liveSessionId = "session-1";

    const { result } = renderHook(() =>
      useEditorTabs({ sessionId: "session-1", audioExists: true }),
    );

    expect(result.current).toEqual([
      { type: "enhanced", id: "note-1" },
      { type: "raw" },
      { type: "transcript" },
    ]);
  });

  it("includes the transcript tab for active meetings with live segments", () => {
    hoisted.hasTranscript = false;
    hoisted.liveSegments = [{ id: "segment-1" }];
    hoisted.liveSessionId = "session-1";
    hoisted.sessionMode = "active";

    const { result } = renderHook(() =>
      useEditorTabs({ sessionId: "session-1", audioExists: false }),
    );

    expect(result.current).toEqual([
      { type: "enhanced", id: "note-1" },
      { type: "raw" },
      { type: "transcript" },
    ]);
  });

  it("omits the transcript tab for inactive sessions without transcript or audio", () => {
    hoisted.hasTranscript = false;

    const { result } = renderHook(() =>
      useEditorTabs({ sessionId: "session-1", audioExists: false }),
    );

    expect(result.current).toEqual([
      { type: "enhanced", id: "note-1" },
      { type: "raw" },
    ]);
  });
});

function findContextMenu(id: string) {
  const menu = hoisted.nativeContextMenus.find((items) =>
    items.some((item) => "id" in item && item.id === id),
  );
  if (!menu) {
    throw new Error(`Context menu not found: ${id}`);
  }
  return menu;
}

function isMenuItem(
  item: CapturedMenuItem,
): item is Extract<CapturedMenuItem, { id: string }> {
  return "id" in item;
}
