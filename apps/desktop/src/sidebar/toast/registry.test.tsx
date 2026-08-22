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
  isLiveMeetingActive: false,
  hasActiveDownload: false,
  downloadingModel: null,
  activeDownloads: [],
  localSttStatus: null,
  isLocalSttModel: false,
  update: {
    status: null,
    version: null,
    progress: null,
    errorMessage: null,
    downloadStarting: false,
    installing: false,
    downloadUpdate: vi.fn(),
    installUpdate: vi.fn(),
  },
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
    expect(toast?.lifecycle).toEqual({ type: "condition-bound" });
  });

  it("shows required setup even if it was dismissed previously", () => {
    const toast = getToastToShow(
      createToastRegistry({
        ...baseParams,
        hasLLMConfigured: false,
      }),
      (toast) => toast.id === "missing-llm",
    );

    expect(toast?.id).toBe("missing-llm");
  });

  it("keeps the missing transcription provider message short", () => {
    const toast = getToastToShow(
      createToastRegistry({
        ...baseParams,
        hasSttConfigured: false,
      }),
      () => false,
    );

    expect(toast?.id).toBe("missing-stt");
    expect(toast?.description).toBe("Transcription provider needed");
    expect(toast?.primaryAction?.label).toBe("Add");
    expect(toast?.lifecycle).toEqual({ type: "condition-bound" });
  });

  it("suggests signing in before provider setup", () => {
    const toast = getToastToShow(
      createToastRegistry({
        ...baseParams,
        isAuthenticated: false,
        hasLLMConfigured: false,
        hasSttConfigured: false,
      }),
      () => false,
    );

    expect(toast?.id).toBe("sign-in-benefits");
    expect(toast?.description).toBe("Sign in to get the most out of Anarlog");
    expect(toast?.primaryAction?.label).toBe("Sign in");
  });

  it("asks for a usable transcription provider after sign-in is dismissed", () => {
    const toast = getToastToShow(
      createToastRegistry({
        ...baseParams,
        isAuthenticated: false,
        hasProSttConfigured: true,
      }),
      (toast) => toast.id === "sign-in-benefits",
    );

    expect(toast?.id).toBe("missing-stt");
    expect(toast?.description).toBe("Transcription provider needed");
  });

  it("keeps Pro providers usable while authentication is loading", () => {
    const proSttToast = getToastToShow(
      createToastRegistry({
        ...baseParams,
        isAuthenticated: false,
        isAuthLoading: true,
        hasProSttConfigured: true,
      }),
      () => false,
    );
    const proLlmToast = getToastToShow(
      createToastRegistry({
        ...baseParams,
        isAuthenticated: false,
        isAuthLoading: true,
        hasProLlmConfigured: true,
      }),
      () => false,
    );

    expect(proSttToast).toBeNull();
    expect(proLlmToast).toBeNull();
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
      (toast) => toast.id === "sign-in-benefits",
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

  it("uses one permanent dismissal for sign-in and Pro promotions", () => {
    const toast = getToastToShow(
      createToastRegistry({
        ...baseParams,
        isAuthenticated: false,
      }),
      (candidate) =>
        candidate.lifecycle.type === "persistent" &&
        candidate.lifecycle.dismissalId === "auth-promotion",
    );

    expect(toast).toBeNull();
  });

  it("offers an available desktop update with a one-day snooze", () => {
    const downloadUpdate = vi.fn();
    const toast = getToastToShow(
      createToastRegistry({
        ...baseParams,
        update: {
          ...baseParams.update,
          status: "available",
          version: "1.0.34",
          downloadUpdate,
        },
      }),
      () => false,
    );

    expect(toast).toMatchObject({
      id: "desktop-update:1.0.34:available",
      description: "Anarlog 1.0.34 is available",
      lifecycle: { type: "persistent", dismissal: "day" },
      primaryAction: { label: "Download" },
    });

    toast?.primaryAction?.onClick();
    expect(downloadUpdate).toHaveBeenCalledOnce();
  });

  it("hides the desktop update toast while a meeting is recording", () => {
    const toast = getToastToShow(
      createToastRegistry({
        ...baseParams,
        isLiveMeetingActive: true,
        update: {
          ...baseParams.update,
          status: "available",
          version: "1.0.34",
        },
      }),
      () => false,
    );

    expect(toast).toBeNull();
  });

  it("lets users dismiss a model download toast for the current download", () => {
    const toast = getToastToShow(
      createToastRegistry({
        ...baseParams,
        hasActiveDownload: true,
        downloadingModel: "apple-speech",
        activeDownloads: [
          { model: "apple-speech", displayName: "apple-speech", progress: 0 },
        ],
      }),
      () => false,
    );

    expect(toast).toMatchObject({
      id: "downloading-model",
      description: "Downloading apple-speech",
      lifecycle: { type: "persistent", dismissal: "session" },
      loading: true,
    });
  });

  it("keeps desktop update progress in the toast", () => {
    const toast = getToastToShow(
      createToastRegistry({
        ...baseParams,
        update: {
          ...baseParams.update,
          status: "downloading",
          version: "1.0.34",
          progress: 0.58,
        },
      }),
      () => false,
    );

    expect(toast).toMatchObject({
      id: "desktop-update:1.0.34:downloading",
      description: "Downloading Anarlog 1.0.34 (58%)",
      lifecycle: { type: "persistent", dismissal: "session" },
      loading: true,
    });
    expect(toast?.primaryAction).toBeUndefined();
  });

  it("offers a ready desktop update without a spinner", () => {
    const toast = getToastToShow(
      createToastRegistry({
        ...baseParams,
        update: {
          ...baseParams.update,
          status: "ready",
          version: "1.0.34",
        },
      }),
      () => false,
    );

    expect(toast).toMatchObject({
      id: "desktop-update:1.0.34:ready",
      description: "Anarlog 1.0.34 is ready to install",
      lifecycle: { type: "persistent", dismissal: "session" },
      primaryAction: { label: "Restart" },
    });
    expect(toast?.loading).toBeUndefined();
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
    expect(languageModelToast.lifecycle).toEqual({ type: "condition-bound" });
    const transcriptionModelToast = createDevtoolsToastPreview({
      preview: "transcription-model",
      onSignIn: vi.fn(),
      onOpenLLMSettings: vi.fn(),
      onOpenSTTSettings: vi.fn(),
    });
    expect(transcriptionModelToast.description).toBe(
      "Transcription provider needed",
    );
    expect(transcriptionModelToast.lifecycle).toEqual({
      type: "condition-bound",
    });
    expect(downloadToast.id).toBe("devtools-downloading-model");
    expect(downloadToast.loading).toBe(true);
    expect(downloadToast.lifecycle).toEqual({
      type: "persistent",
      dismissal: "session",
    });
  });
});
