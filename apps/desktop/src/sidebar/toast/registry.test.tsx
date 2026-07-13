import { describe, expect, it, vi } from "vitest";

import {
  MEETING_DISCLOSURE_AUTO_POST_TOAST_ID,
  createDevtoolsToastPreview,
  createToastRegistry,
  getToastToShow,
} from "./registry";

const baseParams = {
  isAuthenticated: true,
  isAuthLoading: false,
  hasLLMConfigured: true,
  hasSttConfigured: true,
  meetingDisclosureAutoPostEnabled: true,
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
  onEnableMeetingDisclosureAutoPost: vi.fn(),
  onDismissMeetingDisclosureAutoPost: vi.fn(),
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

  it("offers recording disclosure auto-post once when meeting AI is configured", () => {
    const toast = getToastToShow(
      createToastRegistry({
        ...baseParams,
        meetingDisclosureAutoPostEnabled: false,
      }),
      () => false,
    );

    expect(toast?.id).toBe(MEETING_DISCLOSURE_AUTO_POST_TOAST_ID);
    expect(toast?.description).toBe(
      "Auto-post a recording disclosure in Slack Huddles?",
    );
    expect(toast?.actions?.map((action) => action.label)).toEqual([
      "Enable",
      "Not now",
    ]);
    expect(toast?.dismissible).toBe(false);
  });

  it("does not offer recording disclosure auto-post when enabled or dismissed", () => {
    const enabledToast = getToastToShow(
      createToastRegistry(baseParams),
      () => false,
    );
    const dismissedToast = getToastToShow(
      createToastRegistry({
        ...baseParams,
        meetingDisclosureAutoPostEnabled: false,
      }),
      (id) => id === MEETING_DISCLOSURE_AUTO_POST_TOAST_ID,
    );

    expect(enabledToast).toBeNull();
    expect(dismissedToast).toBeNull();
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
