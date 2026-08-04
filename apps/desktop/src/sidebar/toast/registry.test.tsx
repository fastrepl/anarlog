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
  cloudsyncInitialSyncToastId: null,
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
      (id) => id === "sign-in-benefits",
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

  it("shows a dismissible loading toast during initial cloud sync", () => {
    const toast = getToastToShow(
      createToastRegistry({
        ...baseParams,
        cloudsyncInitialSyncToastId: "cloudsync-initial-sync-user-1",
      }),
      () => false,
    );

    expect(toast?.id).toBe("cloudsync-initial-sync-user-1");
    expect(toast?.description).toBe("Syncing your data in the background...");
    expect(toast?.dismissible).toBe(true);
    expect(toast?.loading).toBe(true);
  });

  it("renders the pro upgrade toast without an icon", () => {
    const toast = getToastToShow(
      createToastRegistry({
        ...baseParams,
        isAuthenticated: false,
      }),
      (id) => id === "sign-in-benefits",
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

  it("offers an available desktop update as a dismissible toast", () => {
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
      id: "desktop-update:1.0.34",
      description: "Anarlog 1.0.34 is available",
      dismissible: true,
      primaryAction: { label: "Download" },
    });

    toast?.primaryAction?.onClick();
    expect(downloadUpdate).toHaveBeenCalledOnce();
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
      id: "desktop-update:1.0.34",
      description: "Downloading Anarlog 1.0.34 (58%)",
      dismissible: true,
      loading: true,
    });
    expect(toast?.primaryAction).toBeUndefined();
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
    const transcriptionModelToast = createDevtoolsToastPreview({
      preview: "transcription-model",
      onSignIn: vi.fn(),
      onOpenLLMSettings: vi.fn(),
      onOpenSTTSettings: vi.fn(),
    });
    expect(transcriptionModelToast.description).toBe(
      "Transcription provider needed",
    );
    expect(downloadToast.id).toBe("devtools-downloading-model");
    expect(downloadToast.loading).toBe(true);
  });
});
