import { cleanup, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { LLMConnectionStatus } from "~/ai/hooks";

const hoisted = vi.hoisted(() => ({
  hasTranscript: true,
  sessionMode: "inactive",
  enhancedNoteIds: [] as string[],
  selectedTemplateId: "template-1" as string | undefined,
  llmStatus: {
    status: "success",
    providerId: "hyprnote",
    isHosted: true,
  } as LLMConnectionStatus,
  service: {
    ensureNote: vi.fn(),
    queueAutoEnhanceIfSummaryEmpty: vi.fn(),
  },
}));

vi.mock("../components/shared", () => ({
  useHasTranscript: () => hoisted.hasTranscript,
}));

vi.mock("~/ai/hooks", () => ({
  useLLMConnectionStatus: () => hoisted.llmStatus,
}));

vi.mock("~/ai/contexts", () => ({
  useAITask: vi.fn(),
}));

vi.mock("~/services/enhancer", () => ({
  getEnhancerService: () => hoisted.service,
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  INDEXES: {
    enhancedNotesBySession: "enhancedNotesBySession",
  },
  UI: {
    useSliceRowIds: () => hoisted.enhancedNoteIds,
  },
}));

vi.mock("~/store/tinybase/store/settings", () => ({
  STORE_ID: "settings",
  UI: {
    useValue: () => hoisted.selectedTemplateId,
  },
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (selector: (state: unknown) => unknown) =>
    selector({
      getSessionMode: () => hoisted.sessionMode,
    }),
}));

import { useEnsureDefaultSummary } from "./useEnhancedNotes";

describe("useEnsureDefaultSummary", () => {
  beforeEach(() => {
    cleanup();
    hoisted.hasTranscript = true;
    hoisted.sessionMode = "inactive";
    hoisted.enhancedNoteIds = [];
    hoisted.selectedTemplateId = "template-1";
    hoisted.llmStatus = {
      status: "success",
      providerId: "hyprnote",
      isHosted: true,
    };
    hoisted.service.ensureNote.mockClear();
    hoisted.service.queueAutoEnhanceIfSummaryEmpty.mockClear();
  });

  it("creates the summary row without queueing generation on open", async () => {
    renderHook(() => useEnsureDefaultSummary("session-1"));

    await waitFor(() => {
      expect(hoisted.service.ensureNote).toHaveBeenCalledWith(
        "session-1",
        "template-1",
      );
    });
    expect(
      hoisted.service.queueAutoEnhanceIfSummaryEmpty,
    ).not.toHaveBeenCalled();
  });

  it("creates the summary row without queueing generation before transcript exists", async () => {
    hoisted.hasTranscript = false;

    renderHook(() => useEnsureDefaultSummary("session-1"));

    await waitFor(() => {
      expect(hoisted.service.ensureNote).toHaveBeenCalledWith(
        "session-1",
        "template-1",
      );
    });
    expect(
      hoisted.service.queueAutoEnhanceIfSummaryEmpty,
    ).not.toHaveBeenCalled();
  });

  it("creates the summary row when a hosted subscription blocks generation", async () => {
    hoisted.llmStatus = {
      status: "error",
      reason: "not_pro",
      providerId: "hyprnote",
    };

    renderHook(() => useEnsureDefaultSummary("session-1"));

    await waitFor(() => {
      expect(hoisted.service.ensureNote).toHaveBeenCalledWith(
        "session-1",
        "template-1",
      );
    });
    expect(
      hoisted.service.queueAutoEnhanceIfSummaryEmpty,
    ).not.toHaveBeenCalled();
  });

  it("creates the summary row when provider API keys are missing", async () => {
    hoisted.llmStatus = {
      status: "error",
      reason: "missing_config",
      providerId: "openai",
      missing: ["api_key"],
    };

    renderHook(() => useEnsureDefaultSummary("session-1"));

    await waitFor(() => {
      expect(hoisted.service.ensureNote).toHaveBeenCalledWith(
        "session-1",
        "template-1",
      );
    });
    expect(
      hoisted.service.queueAutoEnhanceIfSummaryEmpty,
    ).not.toHaveBeenCalled();
  });

  it("creates the summary row when model selection is pending", async () => {
    hoisted.llmStatus = {
      status: "pending",
      reason: "missing_model",
      providerId: "hyprnote",
    };

    renderHook(() => useEnsureDefaultSummary("session-1"));

    await waitFor(() => {
      expect(hoisted.service.ensureNote).toHaveBeenCalledWith(
        "session-1",
        "template-1",
      );
    });
    expect(
      hoisted.service.queueAutoEnhanceIfSummaryEmpty,
    ).not.toHaveBeenCalled();
  });
});
