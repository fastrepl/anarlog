import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { Enhanced as SessionEnhanced } from "./index";

import type { LLMConnectionStatus } from "~/ai/hooks";

const hoisted = vi.hoisted(() => ({
  enhanceTask: undefined as
    | {
        status: string;
        error: Error | undefined;
        streamedText: string;
        currentStep: unknown;
        isGenerating: boolean;
      }
    | undefined,
  titleTask: undefined as
    | {
        status: string;
        error: Error | undefined;
        streamedText: string;
        currentStep: unknown;
        isGenerating: boolean;
      }
    | undefined,
  llmStatus: {
    status: "success",
    providerId: "hyprnote",
    isHosted: true,
  } as LLMConnectionStatus,
  content: "",
  noteExists: true,
  sessionTitle: "",
}));

vi.mock("@hypr/ui/components/ui/spinner", () => ({
  Spinner: () => <span data-testid="spinner" />,
}));

vi.mock("streamdown", () => ({
  Streamdown: ({ children }: { children: string }) => <div>{children}</div>,
}));

vi.mock("~/ai/hooks", () => ({
  useAITaskTask: (_taskId: string, taskType: "enhance" | "title") => {
    const task = taskType === "title" ? hoisted.titleTask : hoisted.enhanceTask;

    return {
      status: task?.status ?? "idle",
      error: task?.error,
      streamedText: task?.streamedText ?? "",
      currentStep: task?.currentStep,
      hasTask: !!task,
      isGenerating: task?.isGenerating ?? false,
    };
  },
  useLLMConnectionStatus: () => hoisted.llmStatus,
}));

vi.mock("~/session/queries", () => ({
  useEnhancedNote: () =>
    hoisted.noteExists ? { content: hoisted.content } : null,
}));

vi.mock("./config-error", () => ({
  ConfigError: () => <div>Config error</div>,
}));

vi.mock("./editor", () => ({
  EnhancedEditor: ({
    content,
    contentOverride,
  }: {
    content: string;
    contentOverride?: { content?: unknown[] };
  }) => {
    const collectText = (value: unknown): string => {
      if (!value || typeof value !== "object") {
        return "";
      }

      const node = value as {
        text?: unknown;
        content?: unknown[];
      };

      return [
        typeof node.text === "string" ? node.text : "",
        ...(node.content?.map(collectText) ?? []),
      ].join("");
    };

    return (
      <div>
        <span>Enhanced editor</span>
        <span>{content}</span>
        {contentOverride ? <span>{collectText(contentOverride)}</span> : null}
      </div>
    );
  },
}));

vi.mock("./enhance-error", () => ({
  EnhanceError: () => <div>Enhance error</div>,
}));

function Enhanced({
  sessionId,
  enhancedNoteId,
}: {
  sessionId: string;
  enhancedNoteId: string;
}) {
  return (
    <SessionEnhanced
      sessionId={sessionId}
      sessionTitle={hoisted.sessionTitle}
      enhancedNoteId={enhancedNoteId}
    />
  );
}

describe("Enhanced", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    hoisted.enhanceTask = undefined;
    hoisted.titleTask = undefined;
    hoisted.llmStatus = {
      status: "success",
      providerId: "hyprnote",
      isHosted: true,
    };
    hoisted.content = "";
    hoisted.noteExists = true;
    hoisted.sessionTitle = "";
  });

  it("renders an empty editor before the auto-enhance task is visible", () => {
    render(<Enhanced sessionId="session-1" enhancedNoteId="note-1" />);

    expect(screen.getByText("Enhanced editor")).not.toBeNull();
    expect(screen.queryByRole("status")).toBeNull();
    expect(screen.queryByText("Preparing summary...")).toBeNull();
    expect(screen.queryByTestId("spinner")).toBeNull();
  });

  it("shows a generating status before streamed text arrives", () => {
    hoisted.enhanceTask = {
      status: "generating",
      error: undefined,
      streamedText: "",
      currentStep: undefined,
      isGenerating: true,
    };

    render(<Enhanced sessionId="session-1" enhancedNoteId="note-1" />);

    expect(screen.queryByText("Enhanced editor")).toBeNull();
    expect(screen.getByRole("status")).not.toBeNull();
    expect(screen.getByText("Analyzing structure...")).not.toBeNull();
    expect(
      screen.getByText("Tip: The Anarlog team loves our users!"),
    ).not.toBeNull();
  });

  it("renders streamed summary in the generating view", () => {
    hoisted.enhanceTask = {
      status: "generating",
      error: undefined,
      streamedText: "Streaming summary",
      currentStep: undefined,
      isGenerating: true,
    };

    render(<Enhanced sessionId="session-1" enhancedNoteId="note-1" />);

    expect(screen.queryByText("Enhanced editor")).toBeNull();
    expect(screen.getByText("Streaming summary")).not.toBeNull();
    expect(screen.getByTestId("summary-title-space")).not.toBeNull();
    expect(screen.getByText("Generating title...")).not.toBeNull();
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("keeps the title row while streaming for an already titled session", () => {
    hoisted.sessionTitle = "Existing title";
    hoisted.enhanceTask = {
      status: "generating",
      error: undefined,
      streamedText: "Streaming summary",
      currentStep: undefined,
      isGenerating: true,
    };

    render(<Enhanced sessionId="session-1" enhancedNoteId="note-1" />);

    expect(screen.getByText("Streaming summary")).not.toBeNull();
    expect(screen.getByTestId("summary-title-space")).not.toBeNull();
    expect(screen.getByText("Existing title")).not.toBeNull();
    expect(screen.queryByText("Generating title...")).toBeNull();
  });

  it("shows the generated title while the summary is still streaming", () => {
    hoisted.enhanceTask = {
      status: "generating",
      error: undefined,
      streamedText: "Streaming summary",
      currentStep: undefined,
      isGenerating: true,
    };
    hoisted.titleTask = {
      status: "success",
      error: undefined,
      streamedText: "Generated Session Title",
      currentStep: undefined,
      isGenerating: false,
    };

    render(<Enhanced sessionId="session-1" enhancedNoteId="note-1" />);

    expect(screen.getByTestId("summary-title-space")).not.toBeNull();
    expect(screen.getByText("Generated Session Title")).not.toBeNull();
    expect(screen.queryByText("Generating title...")).toBeNull();
  });

  it("renders the editor after an empty enhance task returns idle", () => {
    hoisted.enhanceTask = {
      status: "idle",
      error: undefined,
      streamedText: "",
      currentStep: undefined,
      isGenerating: false,
    };

    render(<Enhanced sessionId="session-1" enhancedNoteId="note-1" />);

    expect(screen.getByText("Enhanced editor")).not.toBeNull();
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("shows config errors for hosted subscription blockers", () => {
    hoisted.llmStatus = {
      status: "error",
      reason: "not_pro",
      providerId: "hyprnote",
    };

    render(<Enhanced sessionId="session-1" enhancedNoteId="note-1" />);

    expect(screen.getByText("Config error")).not.toBeNull();
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("shows config errors when hosted generation requires authentication", () => {
    hoisted.llmStatus = {
      status: "error",
      reason: "unauthenticated",
      providerId: "hyprnote",
    };

    render(<Enhanced sessionId="session-1" enhancedNoteId="note-1" />);

    expect(screen.getByText("Config error")).not.toBeNull();
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("shows config errors when a provider API key is missing", () => {
    hoisted.llmStatus = {
      status: "error",
      reason: "missing_config",
      providerId: "openai",
      missing: ["api_key"],
    };

    render(<Enhanced sessionId="session-1" enhancedNoteId="note-1" />);

    expect(screen.getByText("Config error")).not.toBeNull();
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("shows config errors when a provider base URL is missing", () => {
    hoisted.llmStatus = {
      status: "error",
      reason: "missing_config",
      providerId: "openai",
      missing: ["base_url"],
    };

    render(<Enhanced sessionId="session-1" enhancedNoteId="note-1" />);

    expect(screen.getByText("Config error")).not.toBeNull();
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("does not show config errors for missing provider setup", () => {
    hoisted.llmStatus = { status: "pending", reason: "missing_provider" };

    render(<Enhanced sessionId="session-1" enhancedNoteId="note-1" />);

    expect(screen.queryByText("Config error")).toBeNull();
    expect(screen.getByText("Enhanced editor")).not.toBeNull();
  });

  it("renders the editor when the enhanced note already has content", () => {
    hoisted.content = JSON.stringify({
      type: "doc",
      content: [{ type: "paragraph", content: [{ type: "text", text: "Hi" }] }],
    });

    render(<Enhanced sessionId="session-1" enhancedNoteId="note-1" />);

    expect(screen.getByText("Enhanced editor")).not.toBeNull();
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("waits for SQLite hydration and mounts the editor with stored content", () => {
    hoisted.noteExists = false;
    const view = render(
      <Enhanced sessionId="session-1" enhancedNoteId="note-1" />,
    );

    expect(screen.queryByText("Enhanced editor")).toBeNull();

    hoisted.noteExists = true;
    hoisted.content = "Stored summary";
    view.rerender(<Enhanced sessionId="session-1" enhancedNoteId="note-1" />);

    expect(screen.getByText("Enhanced editor")).not.toBeNull();
    expect(screen.getByText("Stored summary")).not.toBeNull();
  });
});
