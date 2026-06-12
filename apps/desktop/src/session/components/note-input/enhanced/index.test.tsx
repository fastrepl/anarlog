import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { Enhanced } from "./index";

import type { LLMConnectionStatus } from "~/ai/hooks";

const hoisted = vi.hoisted(() => ({
  task: undefined as
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
}));

vi.mock("streamdown", () => ({
  Streamdown: ({ children }: { children: string }) => <div>{children}</div>,
}));

vi.mock("@hypr/ui/components/ui/spinner", () => ({
  Spinner: () => <span data-testid="spinner" />,
}));

vi.mock("~/ai/hooks", () => ({
  useAITaskTask: () => ({
    status: hoisted.task?.status ?? "idle",
    error: hoisted.task?.error,
    streamedText: hoisted.task?.streamedText ?? "",
    currentStep: hoisted.task?.currentStep,
    hasTask: !!hoisted.task,
    isGenerating: hoisted.task?.isGenerating ?? false,
  }),
  useLLMConnectionStatus: () => hoisted.llmStatus,
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  UI: {
    useCell: () => hoisted.content,
  },
}));

vi.mock("./config-error", () => ({
  ConfigError: () => <div>Config error</div>,
}));

vi.mock("./editor", () => ({
  EnhancedEditor: () => <div>Enhanced editor</div>,
}));

vi.mock("./enhance-error", () => ({
  EnhanceError: () => <div>Enhance error</div>,
}));

describe("Enhanced", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    hoisted.task = undefined;
    hoisted.llmStatus = {
      status: "success",
      providerId: "hyprnote",
      isHosted: true,
    };
    hoisted.content = "";
  });

  it("shows an interim summary state before the enhance task is visible", () => {
    render(<Enhanced sessionId="session-1" enhancedNoteId="note-1" />);

    expect(screen.getByRole("status").textContent).toContain(
      "Preparing summary...",
    );
    expect(screen.getByRole("status").textContent).toContain(
      "Tip: The Anarlog team loves our users!",
    );
    expect(screen.queryByText("Enhanced editor")).toBeNull();
  });

  it("renders the editor after an empty enhance task returns idle", () => {
    hoisted.task = {
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

  it("keeps config errors ahead of the empty interim state", () => {
    hoisted.llmStatus = { status: "pending", reason: "missing_provider" };

    render(<Enhanced sessionId="session-1" enhancedNoteId="note-1" />);

    expect(screen.getByText("Config error")).not.toBeNull();
    expect(screen.queryByRole("status")).toBeNull();
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
});
