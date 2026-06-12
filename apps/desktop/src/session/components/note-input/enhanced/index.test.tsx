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

  it("renders an empty editor before the auto-enhance task is visible", () => {
    render(<Enhanced sessionId="session-1" enhancedNoteId="note-1" />);

    expect(screen.getByText("Enhanced editor")).not.toBeNull();
    expect(screen.queryByRole("status")).toBeNull();
    expect(screen.queryByText("Preparing summary...")).toBeNull();
    expect(screen.queryByTestId("spinner")).toBeNull();
  });

  it("keeps the generating view quiet until summary text streams", () => {
    hoisted.task = {
      status: "generating",
      error: undefined,
      streamedText: "",
      currentStep: undefined,
      isGenerating: true,
    };

    render(<Enhanced sessionId="session-1" enhancedNoteId="note-1" />);

    expect(screen.queryByRole("status")).toBeNull();
    expect(screen.queryByText("Preparing summary...")).toBeNull();
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
});
