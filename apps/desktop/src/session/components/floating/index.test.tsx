import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { FloatingActionButton } from "./index";

import type { LLMConnectionStatus } from "~/ai/hooks";
import type { Tab } from "~/store/zustand/tabs";
import type { EditorView } from "~/store/zustand/tabs/schema";

const hoisted = vi.hoisted(() => ({
  currentTab: { type: "raw" } as
    | { type: "raw" }
    | { type: "transcript" }
    | {
        type: "enhanced";
        id: string;
      },
  hasTranscript: true,
  enhanceTaskStatus: undefined as string | undefined,
  enhancedContent: "Generated summary",
  templateId: undefined as string | undefined,
  llmStatus: {
    status: "success",
    providerId: "hyprnote",
    isHosted: true,
  } as LLMConnectionStatus,
  isCaretNearBottom: false,
  sessionMode: "inactive",
  sendEvent: vi.fn(),
  enhance: vi.fn(),
  regenerateSummary: vi.fn(),
  cancelSummary: vi.fn(),
  regenerateTranscript: vi.fn(),
  stopListening: vi.fn(),
  stopTranscription: vi.fn(),
  requestMainListenerControl: vi.fn(),
  isMainWebviewWindow: true,
  updateSessionTabState: vi.fn(),
}));

vi.mock("@hypr/ui/components/ui/spinner", () => ({
  Spinner: () => <span data-testid="spinner" />,
}));

vi.mock("./listen", () => ({
  ListenButton: () => <button type="button">Start listening</button>,
}));

vi.mock("~/shared/chat-cta", () => ({
  ChatCTA: () => (
    <button type="button" onClick={() => hoisted.sendEvent({ type: "OPEN" })}>
      Ask Anarlog anything
    </button>
  ),
}));

vi.mock("~/session/components/shared", () => ({
  useCurrentNoteTab: () => hoisted.currentTab,
  useHasTranscript: () => hoisted.hasTranscript,
  hasStoredNoteContent: (value: unknown) =>
    typeof value === "string" && value.trim().length > 0,
}));

vi.mock("~/ai/contexts", () => ({
  useAITask: (
    selector: (state: {
      tasks: Record<string, { status: string | undefined }>;
    }) => unknown,
  ) =>
    selector({
      tasks: hoisted.enhanceTaskStatus
        ? { "note-1-enhance": { status: hoisted.enhanceTaskStatus } }
        : {},
    }),
}));

vi.mock("~/ai/hooks", () => ({
  useLLMConnectionStatus: () => hoisted.llmStatus,
  useLanguageModel: () => "model",
  useAITaskTask: () => ({
    isIdle:
      hoisted.enhanceTaskStatus === undefined ||
      hoisted.enhanceTaskStatus === "idle",
    isGenerating: hoisted.enhanceTaskStatus === "generating",
    isError: hoisted.enhanceTaskStatus === "error",
    error: null,
    start: hoisted.regenerateSummary,
    cancel: hoisted.cancelSummary,
  }),
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  UI: {
    useCell: (_table: string, _row: string, cell: string) => {
      if (cell === "content") {
        return hoisted.enhancedContent;
      }

      if (cell === "template_id") {
        return hoisted.templateId;
      }

      return undefined;
    },
  },
}));

vi.mock("~/services/enhancer", () => ({
  getEnhancerService: () => ({
    enhance: hoisted.enhance,
  }),
}));

vi.mock("~/session/components/note-input/transcript/actions", () => ({
  useRegenerateTranscript: () => hoisted.regenerateTranscript,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: unknown) => unknown) =>
    selector({ updateSessionTabState: hoisted.updateSessionTabState }),
}));

vi.mock("../caret-position-context", () => ({
  useCaretPosition: () => ({
    isCaretNearBottom: hoisted.isCaretNearBottom,
  }),
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (
    selector: (state: {
      getSessionMode: () => string;
      stop: () => void;
      stopTranscription: (sessionId: string) => void;
    }) => unknown,
  ) =>
    selector({
      getSessionMode: () => hoisted.sessionMode,
      stop: hoisted.stopListening,
      stopTranscription: hoisted.stopTranscription,
    }),
}));

vi.mock("~/stt/window-control", () => ({
  isMainWebviewWindow: () => hoisted.isMainWebviewWindow,
  requestMainListenerControl: hoisted.requestMainListenerControl,
}));

describe("FloatingActionButton", () => {
  const tab = {
    type: "sessions",
    id: "session-1",
    active: true,
    pinned: false,
    slotId: "slot-1",
    state: { view: null, autoStart: null },
  } as Extract<Tab, { type: "sessions" }>;
  const renderFloatingActionButton = (
    props: Partial<React.ComponentProps<typeof FloatingActionButton>> = {},
  ) =>
    render(
      <FloatingActionButton
        currentView={hoisted.currentTab as EditorView}
        tab={tab}
        {...props}
      />,
    );

  beforeEach(() => {
    hoisted.currentTab = { type: "raw" };
    hoisted.hasTranscript = true;
    hoisted.enhanceTaskStatus = undefined;
    hoisted.enhancedContent = "Generated summary";
    hoisted.templateId = undefined;
    hoisted.llmStatus = {
      status: "success",
      providerId: "hyprnote",
      isHosted: true,
    };
    hoisted.isCaretNearBottom = false;
    hoisted.sessionMode = "inactive";
    hoisted.sendEvent.mockClear();
    hoisted.enhance.mockReset();
    hoisted.regenerateSummary.mockReset();
    hoisted.cancelSummary.mockReset();
    hoisted.regenerateTranscript.mockReset();
    hoisted.stopListening.mockReset();
    hoisted.stopTranscription.mockReset();
    hoisted.requestMainListenerControl.mockReset();
    hoisted.isMainWebviewWindow = true;
    hoisted.updateSessionTabState.mockClear();
  });

  afterEach(() => {
    cleanup();
  });

  it("shows the chat FAB on raw memo view after transcript exists", () => {
    renderFloatingActionButton();

    expect(
      screen.queryByRole("button", { name: "Ask Anarlog anything" }),
    ).not.toBeNull();
  });

  it("shows chat instead of a regenerate summary FAB on generated summaries", () => {
    hoisted.currentTab = { type: "enhanced", id: "note-1" };

    renderFloatingActionButton();

    expect(
      screen.queryByRole("button", { name: "Ask Anarlog anything" }),
    ).not.toBeNull();
    expect(
      screen.queryByRole("button", { name: "Regenerate summary" }),
    ).toBeNull();
  });

  it("shows a generate summary FAB instead of chat for empty transcript-backed summaries", () => {
    hoisted.currentTab = { type: "enhanced", id: "note-1" };
    hoisted.enhancedContent = "";
    hoisted.templateId = "template-1";
    hoisted.enhance.mockReturnValue({
      type: "started",
      noteId: "note-1",
    });

    renderFloatingActionButton();

    expect(
      screen.queryByRole("button", { name: "Ask Anarlog anything" }),
    ).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Generate summary" }));

    expect(hoisted.enhance).toHaveBeenCalledWith("session-1", {
      templateId: "template-1",
    });
    expect(hoisted.updateSessionTabState).toHaveBeenCalledWith(tab, {
      ...tab.state,
      view: { type: "enhanced", id: "note-1" },
    });
  });

  it("keeps the generate summary FAB visible after an empty enhance success", () => {
    hoisted.currentTab = { type: "enhanced", id: "note-1" };
    hoisted.enhanceTaskStatus = "success";
    hoisted.enhancedContent = "";

    renderFloatingActionButton();

    expect(
      screen.queryByRole("button", { name: "Ask Anarlog anything" }),
    ).toBeNull();
    expect(
      screen.getByRole("button", { name: "Generate summary" }),
    ).not.toBeNull();
  });

  it("hides the chat FAB when the visible summary has a generation issue", () => {
    hoisted.currentTab = { type: "enhanced", id: "note-1" };
    hoisted.enhanceTaskStatus = "error";

    renderFloatingActionButton();

    expect(
      screen.queryByRole("button", { name: "Ask Anarlog anything" }),
    ).toBeNull();
  });

  it("hides the chat FAB when the visible summary has a setup issue", () => {
    hoisted.currentTab = { type: "enhanced", id: "note-1" };
    hoisted.enhanceTaskStatus = "idle";
    hoisted.enhancedContent = "";
    hoisted.llmStatus = { status: "pending", reason: "missing_provider" };

    renderFloatingActionButton();

    expect(
      screen.queryByRole("button", { name: "Ask Anarlog anything" }),
    ).toBeNull();
  });

  it("hides the FAB while the visible summary is generating", () => {
    hoisted.currentTab = { type: "enhanced", id: "note-1" };
    hoisted.enhanceTaskStatus = "generating";
    hoisted.enhancedContent = "";
    hoisted.llmStatus = { status: "pending", reason: "missing_provider" };

    renderFloatingActionButton();

    expect(
      screen.queryByRole("button", { name: "Ask Anarlog anything" }),
    ).toBeNull();
    expect(screen.queryByRole("button", { name: "Stop summary" })).toBeNull();
  });

  it("keeps the chat FAB visible near the editor caret", () => {
    hoisted.isCaretNearBottom = true;

    renderFloatingActionButton();

    const wrapper = screen.getByText("Ask Anarlog anything").parentElement;
    const hoverZone = wrapper?.parentElement;

    expect(hoverZone?.className).toContain("pointer-events-none");
    expect(hoverZone?.className).toContain("bottom-3");
    expect(hoverZone?.className).not.toContain("-bottom-4");
    expect(hoverZone?.className).toContain("h-10");
    expect(hoverZone?.className).toContain("w-40");
    expect(hoverZone?.className).toContain("pb-0");
    expect(wrapper?.getAttribute("aria-hidden")).toBe("false");
    expect(wrapper?.style.getPropertyValue("--floating-fab-tuck-offset")).toBe(
      "0px",
    );
    expect(wrapper?.className).toContain("pointer-events-auto");
    expect(wrapper?.className).not.toContain("group-hover:translate-y-0");
  });

  it("keeps the chat FAB visible during active meetings", () => {
    hoisted.sessionMode = "active";

    renderFloatingActionButton();

    const wrapper = screen.getByText("Ask Anarlog anything").parentElement;
    const hoverZone = wrapper?.parentElement;

    expect(hoverZone?.className).toContain("pointer-events-none");
    expect(hoverZone?.className).toContain("bottom-3");
    expect(hoverZone?.className).not.toContain("-bottom-4");
    expect(hoverZone?.className).toContain("h-10");
    expect(hoverZone?.className).toContain("w-40");
    expect(hoverZone?.className).toContain("pb-0");
    expect(wrapper?.getAttribute("aria-hidden")).toBe("false");
    expect(wrapper?.style.getPropertyValue("--floating-fab-tuck-offset")).toBe(
      "0px",
    );
    expect(wrapper?.className).toContain("pointer-events-auto");
    expect(wrapper?.className).not.toContain("group-hover:pointer-events-auto");
    expect(wrapper?.className).not.toContain("before:pointer-events-none");
  });

  it("shows the chat FAB during active meetings before transcript exists", () => {
    hoisted.sessionMode = "active";
    hoisted.hasTranscript = false;

    renderFloatingActionButton();

    const wrapper = screen.getByText("Ask Anarlog anything").parentElement;

    expect(
      screen.queryByRole("button", { name: "Start listening" }),
    ).toBeNull();
    expect(wrapper?.getAttribute("aria-hidden")).toBe("false");
    expect(wrapper?.style.getPropertyValue("--floating-fab-tuck-offset")).toBe(
      "0px",
    );
  });

  it("opens chat from the active meeting FAB", () => {
    hoisted.sessionMode = "active";

    renderFloatingActionButton();

    fireEvent.click(
      screen.getByRole("button", { name: "Ask Anarlog anything" }),
    );

    expect(hoisted.sendEvent).toHaveBeenCalledWith({ type: "OPEN" });
  });

  it("shows a finalizing indicator after live listening stops", () => {
    hoisted.sessionMode = "finalizing";

    renderFloatingActionButton();

    const button = screen.getByRole("button", {
      name: "Preparing transcript...",
    });

    expect(button.hasAttribute("disabled")).toBe(true);
    expect(screen.getByTestId("spinner")).not.toBeNull();
    expect(
      screen.queryByRole("button", { name: "Ask Anarlog anything" }),
    ).toBeNull();
  });

  it("tucks the listen FAB near the editor caret instead of scroll state", () => {
    hoisted.hasTranscript = false;
    hoisted.isCaretNearBottom = true;

    renderFloatingActionButton();

    const wrapper = screen.getByText("Start listening").parentElement;

    expect(wrapper?.getAttribute("aria-hidden")).toBe("true");
    expect(wrapper?.style.getPropertyValue("--floating-fab-tuck-offset")).toBe(
      "calc(100% - 0.5rem + 18px)",
    );
  });

  it("hides the listen FAB when listening is disabled", () => {
    hoisted.hasTranscript = false;

    renderFloatingActionButton({ allowListening: false });

    expect(
      screen.queryByRole("button", { name: "Start listening" }),
    ).toBeNull();
  });

  it("shows a regenerate transcript FAB on empty transcript views backed by audio", () => {
    hoisted.currentTab = { type: "transcript" };
    hoisted.hasTranscript = false;

    renderFloatingActionButton({ audioExists: true });

    expect(
      screen.queryByRole("button", { name: "Ask Anarlog anything" }),
    ).toBeNull();

    fireEvent.click(
      screen.getByRole("button", { name: "Regenerate transcript" }),
    );

    expect(hoisted.regenerateTranscript).toHaveBeenCalledTimes(1);
  });

  it("shows a regenerate transcript FAB on existing transcript views backed by audio", () => {
    hoisted.currentTab = { type: "transcript" };
    hoisted.hasTranscript = true;

    renderFloatingActionButton({ audioExists: true });

    expect(
      screen.queryByRole("button", { name: "Ask Anarlog anything" }),
    ).toBeNull();

    fireEvent.click(
      screen.getByRole("button", { name: "Regenerate transcript" }),
    );

    expect(hoisted.regenerateTranscript).toHaveBeenCalledTimes(1);
  });

  it("shows a stop transcript FAB while batch transcription is running", () => {
    hoisted.currentTab = { type: "transcript" };
    hoisted.sessionMode = "running_batch";

    renderFloatingActionButton({ audioExists: true });

    fireEvent.click(screen.getByRole("button", { name: "Stop transcription" }));

    expect(hoisted.stopTranscription).toHaveBeenCalledWith("session-1");
  });

  it("shows a stop listening FAB while live recording is active", () => {
    hoisted.currentTab = { type: "transcript" };
    hoisted.sessionMode = "active";

    renderFloatingActionButton({ audioExists: true });

    fireEvent.click(screen.getByRole("button", { name: "Stop listening" }));

    expect(hoisted.stopListening).toHaveBeenCalledTimes(1);
    expect(hoisted.requestMainListenerControl).not.toHaveBeenCalled();
  });

  it("delegates live recording stop from standalone windows", () => {
    hoisted.currentTab = { type: "transcript" };
    hoisted.sessionMode = "active";
    hoisted.isMainWebviewWindow = false;

    renderFloatingActionButton({ audioExists: true });

    fireEvent.click(screen.getByRole("button", { name: "Stop listening" }));

    expect(hoisted.requestMainListenerControl).toHaveBeenCalledWith(
      "stop",
      "session-1",
    );
    expect(hoisted.stopListening).not.toHaveBeenCalled();
  });

  it("hides the regenerate transcript FAB when audio is missing", () => {
    hoisted.currentTab = { type: "transcript" };
    hoisted.hasTranscript = false;

    renderFloatingActionButton();

    expect(
      screen.queryByRole("button", { name: "Regenerate transcript" }),
    ).toBeNull();
  });

  it("shows a skip reason in the FAB slot instead of the chat FAB", () => {
    renderFloatingActionButton({
      skipReason: "Not enough words recorded (3/5 minimum)",
    });

    const status = screen.getByRole("status");

    expect(status.textContent).toBe("Not enough words recorded (3/5 minimum)");
    expect(status.className).toContain("text-red-400");
    expect(status.parentElement?.className).toContain("pb-4");
    expect(
      screen.queryByRole("button", { name: "Ask Anarlog anything" }),
    ).toBeNull();
  });

  it("keeps a skip reason visible without tuck behavior", () => {
    renderFloatingActionButton({
      skipReason: "Not enough words recorded (3/5 minimum)",
    });

    const status = screen.getByRole("status");

    expect(status.className).toContain("translate-y-0");
    expect(status.parentElement?.className).not.toContain("group");
  });
});
