import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { NoteInput } from ".";

import type { EditorView } from "~/store/zustand/tabs/schema";

const hoisted = vi.hoisted(() => ({
  editorTabs: [{ type: "raw" }, { type: "transcript" }] as EditorView[],
  hotkeys: [] as Array<{ keys: string; callback: () => void }>,
  onBeforeTabChange: vi.fn(),
  sessionMode: "inactive",
  updateSessionTabState: vi.fn(),
}));

vi.mock("./enhanced", () => ({
  Enhanced: () => <div data-testid="enhanced-editor" />,
}));

vi.mock("./header", () => ({
  Header: ({
    currentTab,
    editorTabs,
    handleTabChange,
    isTranscribing,
  }: {
    currentTab: EditorView;
    editorTabs: EditorView[];
    handleTabChange: (view: EditorView) => void;
    isTranscribing?: boolean;
  }) => (
    <div>
      <div data-testid="current-tab">{formatEditorView(currentTab)}</div>
      <div data-testid="is-transcribing">{String(isTranscribing)}</div>
      {editorTabs.map((editorTab) => (
        <button
          key={formatEditorView(editorTab)}
          type="button"
          onClick={() => handleTabChange(editorTab)}
        >
          {formatEditorView(editorTab)}
        </button>
      ))}
    </div>
  ),
  useEditorTabs: () => hoisted.editorTabs,
}));

vi.mock("./insights", () => ({
  Insights: () => <div data-testid="insights" />,
}));

vi.mock("./raw", () => ({
  RawEditor: () => <div data-testid="raw-editor" />,
}));

vi.mock("./search/bar", () => ({
  SearchBar: () => <div data-testid="search-bar" />,
}));

vi.mock("./search/context", () => ({
  useSearch: () => null,
}));

vi.mock("./transcript", () => ({
  Transcript: () => <div data-testid="transcript" />,
}));

vi.mock("~/session/components/caret-position-context", () => ({
  useCaretNearBottom: vi.fn(),
}));

vi.mock("~/session/components/shared", () => ({
  useCurrentNoteTab: () => ({ type: "raw" }),
}));

vi.mock("~/shared/hooks/useScrollPreservation", () => ({
  useScrollPreservation: () => ({
    onBeforeTabChange: hoisted.onBeforeTabChange,
    scrollRef: { current: null },
  }),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: vi.fn((selector: (state: unknown) => unknown) =>
    selector({
      updateSessionTabState: hoisted.updateSessionTabState,
    }),
  ),
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (
    selector: (state: {
      getSessionMode: (sessionId: string) => string;
    }) => unknown,
  ) =>
    selector({
      getSessionMode: () => hoisted.sessionMode,
    }),
}));

vi.mock("react-hotkeys-hook", () => ({
  useHotkeys: (keys: string, callback: () => void) => {
    hoisted.hotkeys.push({ keys, callback });
  },
}));

function formatEditorView(view: EditorView) {
  return view.type === "enhanced" ? `enhanced:${view.id}` : view.type;
}

function renderNoteInput({
  currentTab = { type: "raw" },
  handleTabChange = vi.fn(),
}: {
  currentTab?: EditorView;
  handleTabChange?: (view: EditorView) => void;
} = {}) {
  return {
    handleTabChange,
    ...render(
      <NoteInput
        tab={{
          active: true,
          id: "session-1",
          pinned: false,
          slotId: "slot-1",
          state: { autoStart: null, view: currentTab },
          type: "sessions",
        }}
        editorTabs={hoisted.editorTabs}
        currentTab={currentTab}
        handleTabChange={handleTabChange}
      />,
    ),
  };
}

describe("NoteInput tab selection", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    hoisted.hotkeys = [];
    hoisted.onBeforeTabChange.mockClear();
    hoisted.sessionMode = "inactive";
    hoisted.updateSessionTabState.mockClear();
  });

  it("does not move the header ahead of the parent tab state", () => {
    const { handleTabChange } = renderNoteInput();

    fireEvent.click(screen.getByRole("button", { name: "transcript" }));

    expect(handleTabChange).toHaveBeenCalledWith({ type: "transcript" });
    expect(screen.getByTestId("current-tab").textContent).toBe("raw");
  });

  it("reflects the parent-selected tab in the header", () => {
    const { rerender, handleTabChange } = renderNoteInput();
    const currentTab = { type: "transcript" } satisfies EditorView;

    rerender(
      <NoteInput
        tab={{
          active: true,
          id: "session-1",
          pinned: false,
          slotId: "slot-1",
          state: { autoStart: null, view: currentTab },
          type: "sessions",
        }}
        editorTabs={hoisted.editorTabs}
        currentTab={currentTab}
        handleTabChange={handleTabChange}
      />,
    );

    expect(screen.getByTestId("current-tab").textContent).toBe("transcript");
  });

  it("does not show the transcript spinner while a meeting is active", () => {
    hoisted.sessionMode = "active";

    renderNoteInput();

    expect(screen.getByTestId("is-transcribing").textContent).toBe("false");
  });

  it("keeps the transcript spinner while finalizing", () => {
    hoisted.sessionMode = "finalizing";

    renderNoteInput();

    expect(screen.getByTestId("is-transcribing").textContent).toBe("true");
  });

  it("keeps the transcript spinner while batch transcription is running", () => {
    hoisted.sessionMode = "running_batch";

    renderNoteInput();

    expect(screen.getByTestId("is-transcribing").textContent).toBe("true");
  });
});
