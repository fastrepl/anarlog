import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { EditorView } from "~/store/zustand/tabs/schema";

const hoisted = vi.hoisted(() => ({
  enhance: vi.fn(),
  userTemplates: [] as Array<{
    id: string;
    title: string;
    description: string;
    pinned: boolean;
    sections: unknown[];
  }>,
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

vi.mock("~/ai/hooks", () => ({
  useAITaskTask: () => ({
    isIdle: true,
    isGenerating: false,
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

vi.mock("~/services/enhancer", () => ({
  getEnhancerService: () => ({ enhance: hoisted.enhance }),
}));

vi.mock("~/shared/hooks/useNativeContextMenu", () => ({
  useNativeContextMenu: () => vi.fn(),
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

vi.mock("~/templates", () => ({
  filterWebTemplatesAgainstUserTemplates: () => [],
  getTemplateCreatorLabel: () => "You",
  parseWebTemplates: () => [],
  useCreateTemplate: () => vi.fn(),
  useTemplateCreatorName: () => "You",
  useUserTemplate: () => ({ data: { title: "Summary" } }),
  useUserTemplates: () => hoisted.userTemplates,
}));

vi.mock("./transcript/export-data", () => ({
  formatTranscriptExportSegments: () => "",
  useTranscriptExportSegments: () => ({ data: [] }),
}));

import { Header } from "./header";

describe("Header", () => {
  beforeEach(() => {
    hoisted.enhance.mockReset();
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

    const summaryTab = screen.getByRole("button", { name: "Summary" });
    const memoTab = screen.getByRole("button", { name: "Memos" });
    const transcriptTab = screen.getByRole("button", { name: "Transcript" });

    expect(summaryTab.getAttribute("data-state")).toBeNull();
    expect(
      screen.getByRole("tablist").getAttribute("data-tauri-drag-region"),
    ).toBe("false");
    expect(summaryTab.getAttribute("aria-current")).toBeNull();
    expect(memoTab.getAttribute("aria-current")).toBe("page");
    expect(summaryTab.textContent).toBe("");
    expect(transcriptTab.textContent).toBe("");
    expect(summaryTab.getAttribute("title")).toBe(
      "Summary was used to generate this summary.",
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

    fireEvent.click(screen.getByRole("button", { name: "Summary" }));

    expect(screen.getByPlaceholderText("Search templates...")).not.toBeNull();

    fireEvent.click(memoTab);

    view.rerender(
      <Header
        sessionId="session-1"
        editorTabs={editorTabs}
        currentTab={{ type: "raw" }}
        handleTabChange={handleTabChange}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Summary" }));

    expect(handleTabChange).toHaveBeenNthCalledWith(2, { type: "raw" });
    expect(handleTabChange).toHaveBeenNthCalledWith(3, {
      type: "enhanced",
      id: "note-1",
    });
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
    fireEvent.click(screen.getByRole("button", { name: "Summary" }));

    expect(handleTabChange).toHaveBeenNthCalledWith(1, { type: "raw" });
    expect(handleTabChange).toHaveBeenNthCalledWith(2, {
      type: "enhanced",
      id: "note-1",
    });
  });

  it("selects the enhanced note returned by the enhancer when changing templates", () => {
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
      noteId: "note-2",
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

    fireEvent.click(screen.getByRole("button", { name: "Summary" }));
    fireEvent.click(screen.getByRole("button", { name: /Decision Log/ }));

    expect(hoisted.enhance).toHaveBeenCalledWith("session-1", {
      templateId: "template-2",
    });
    expect(handleTabChange).toHaveBeenCalledWith({
      type: "enhanced",
      id: "note-2",
    });
  });
});
