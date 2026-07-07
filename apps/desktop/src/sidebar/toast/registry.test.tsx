import { describe, expect, it, vi } from "vitest";

import {
  createDevtoolsToastPreview,
  createToastRegistry,
  getToastToShow,
} from "./registry";

const baseParams = {
  isAuthenticated: true,
  isAuthLoading: false,
  hasLLMConfigured: true,
  hasSttConfigured: true,
  hasProSttConfigured: false,
  hasProLlmConfigured: false,
  isAiTranscriptionTabActive: false,
  isAiIntelligenceTabActive: false,
  isBatchTranscribingInActiveTranscriptTab: false,
  hasActiveDownload: false,
  downloadProgress: null,
  downloadingModel: null,
  activeDownloads: [],
  localSttStatus: null,
  isLocalSttModel: false,
  onSignIn: vi.fn(),
  onOpenLLMSettings: vi.fn(),
  onOpenSTTSettings: vi.fn(),
};

describe("sidebar toast registry", () => {
  it("keeps the missing language model message short", () => {
    const toast = getToastToShow(
      createToastRegistry({
        ...baseParams,
        hasLLMConfigured: false,
      }),
      () => false,
    );

    expect(toast?.id).toBe("missing-llm");
    expect(toast?.description).toBe("Language model needed");
    expect(toast?.primaryAction?.label).toBe("Add");
  });

  it("keeps the missing transcription model message short", () => {
    const toast = getToastToShow(
      createToastRegistry({
        ...baseParams,
        hasSttConfigured: false,
      }),
      () => false,
    );

    expect(toast?.id).toBe("missing-stt");
    expect(toast?.description).toBe("Transcription model needed");
    expect(toast?.primaryAction?.label).toBe("Add");
  });

  it("hides local STT loading while the active transcript tab shows batch progress", () => {
    const toast = getToastToShow(
      createToastRegistry({
        ...baseParams,
        localSttStatus: "loading",
        isLocalSttModel: true,
        isBatchTranscribingInActiveTranscriptTab: true,
      }),
      () => false,
    );

    expect(toast).toBeNull();
  });

  it("shows local STT loading outside active transcript batch progress", () => {
    const toast = getToastToShow(
      createToastRegistry({
        ...baseParams,
        localSttStatus: "loading",
        isLocalSttModel: true,
      }),
      () => false,
    );

    expect(toast?.id).toBe("local-stt-loading");
    expect(toast?.description).toBe("Starting transcription...");
  });

  it("renders the pro upgrade toast without an icon", () => {
    const toast = getToastToShow(
      createToastRegistry({
        ...baseParams,
        isAuthenticated: false,
      }),
      () => false,
    );
    const previewToast = createDevtoolsToastPreview({
      preview: "pro",
      onSignIn: vi.fn(),
      onOpenLLMSettings: vi.fn(),
      onOpenSTTSettings: vi.fn(),
    });

    expect(toast?.id).toBe("upgrade-to-pro");
    expect(toast?.description).toBe("Pro features available");
    expect(toast?.icon).toBeUndefined();
    expect(previewToast.icon).toBeUndefined();
  });

  it("creates devtools previews with app toast content", () => {
    const languageModelToast = createDevtoolsToastPreview({
      preview: "language-model",
      onSignIn: vi.fn(),
      onOpenLLMSettings: vi.fn(),
      onOpenSTTSettings: vi.fn(),
    });
    const downloadToast = createDevtoolsToastPreview({
      preview: "download",
      onSignIn: vi.fn(),
      onOpenLLMSettings: vi.fn(),
      onOpenSTTSettings: vi.fn(),
    });

    expect(languageModelToast.id).toBe("devtools-missing-llm");
    expect(languageModelToast.description).toBe("Language model needed");
    expect(languageModelToast.primaryAction?.label).toBe("Add");
    expect(downloadToast.id).toBe("devtools-downloading-model");
    expect(downloadToast.progress).toBe(42);
  });
});
