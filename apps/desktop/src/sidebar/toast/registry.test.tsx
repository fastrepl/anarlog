import { describe, expect, it, vi } from "vitest";

import { createToastRegistry, getToastToShow } from "./registry";

type RegistryParams = Parameters<typeof createToastRegistry>[0];

const baseParams: RegistryParams = {
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

type Toast = NonNullable<ReturnType<typeof getToastToShow>>;

function showToast(
  overrides: Partial<RegistryParams> = {},
  isDismissed: (toast: Toast) => boolean = () => false,
) {
  return getToastToShow(
    createToastRegistry({ ...baseParams, ...overrides }),
    isDismissed,
  );
}

function withUpdate(update: Partial<RegistryParams["update"]>) {
  return { update: { ...baseParams.update, version: "1.0.34", ...update } };
}

describe("sidebar toast registry", () => {
  it.each([
    ["missing-llm", { hasLLMConfigured: false }],
    ["missing-stt", { hasSttConfigured: false }],
  ] as const)(
    "requires %s setup even if it was dismissed previously",
    (id, overrides) => {
      const toast = showToast(overrides, (toast) => toast.id === id);

      expect(toast?.id).toBe(id);
      expect(toast?.lifecycle).toEqual({ type: "condition-bound" });
    },
  );

  it("suggests signing in before provider setup", () => {
    const onSignIn = vi.fn();
    const toast = showToast({
      isAuthenticated: false,
      hasLLMConfigured: false,
      hasSttConfigured: false,
      onSignIn,
    });

    expect(toast?.id).toBe("sign-in-benefits");
    toast?.primaryAction?.onClick();
    expect(onSignIn).toHaveBeenCalledOnce();
  });

  it("asks for a usable transcription provider after sign-in is dismissed", () => {
    const toast = showToast(
      { isAuthenticated: false, hasProSttConfigured: true },
      (toast) => toast.id === "sign-in-benefits",
    );

    expect(toast?.id).toBe("missing-stt");
  });

  it("promotes Pro after sign-in is dismissed, sharing one permanent dismissal", () => {
    expect(
      showToast(
        { isAuthenticated: false },
        (toast) => toast.id === "sign-in-benefits",
      )?.id,
    ).toBe("upgrade-to-pro");
    expect(
      showToast(
        { isAuthenticated: false },
        (toast) =>
          toast.lifecycle.type === "persistent" &&
          toast.lifecycle.dismissalId === "auth-promotion",
      ),
    ).toBeNull();
  });

  it.each([
    ["STT", { hasProSttConfigured: true }],
    ["LLM", { hasProLlmConfigured: true }],
  ])("keeps Pro %s usable while authentication is loading", (_, overrides) => {
    expect(
      showToast({ isAuthenticated: false, isAuthLoading: true, ...overrides }),
    ).toBeNull();
  });

  it("shows local STT loading unless the transcript tab shows batch progress", () => {
    const loading = {
      localSttStatus: "loading",
      isLocalSttModel: true,
    } as const;

    expect(showToast(loading)?.id).toBe("local-stt-loading");
    expect(
      showToast({ ...loading, isBatchTranscribingInActiveTranscriptTab: true }),
    ).toBeNull();
  });

  it("offers an available desktop update with a one-day snooze", () => {
    const downloadUpdate = vi.fn();
    const toast = showToast(
      withUpdate({ status: "available", downloadUpdate }),
    );

    expect(toast).toMatchObject({
      id: "desktop-update:1.0.34:available",
      lifecycle: { type: "persistent", dismissal: "day" },
    });

    toast?.primaryAction?.onClick();
    expect(downloadUpdate).toHaveBeenCalledOnce();
  });

  it("hides the desktop update toast while a meeting is recording", () => {
    expect(
      showToast({
        isLiveMeetingActive: true,
        ...withUpdate({ status: "available" }),
      }),
    ).toBeNull();
  });

  it("lets users dismiss a model download toast for the current download", () => {
    const toast = showToast({
      hasActiveDownload: true,
      downloadingModel: "apple-speech",
      activeDownloads: [
        { model: "apple-speech", displayName: "apple-speech", progress: 0 },
      ],
    });

    expect(toast).toMatchObject({
      id: "downloading-model",
      lifecycle: { type: "persistent", dismissal: "session" },
      loading: true,
    });
  });

  it("keeps desktop update progress in the toast", () => {
    const toast = showToast(
      withUpdate({ status: "downloading", progress: 0.58 }),
    );

    expect(toast).toMatchObject({
      id: "desktop-update:1.0.34:downloading",
      description: "Downloading Anarlog 1.0.34 (58%)",
      lifecycle: { type: "persistent", dismissal: "session" },
      loading: true,
    });
    expect(toast?.primaryAction).toBeUndefined();
  });

  it("keeps Restart available once ready even if the download mutation is still settling", () => {
    const installUpdate = vi.fn();
    const toast = showToast(
      withUpdate({ status: "ready", downloadStarting: true, installUpdate }),
    );

    expect(toast).toMatchObject({
      id: "desktop-update:1.0.34:ready",
      lifecycle: { type: "persistent", dismissal: "session" },
    });
    expect(toast?.loading).toBeUndefined();

    toast?.primaryAction?.onClick();
    expect(installUpdate).toHaveBeenCalledOnce();
  });
});
