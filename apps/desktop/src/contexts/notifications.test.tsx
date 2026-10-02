import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { DownloadProgressPayload } from "@anlg/plugin-local-stt";

const mocks = vi.hoisted(() => ({
  downloadHandler: null as
    | null
    | ((event: { payload: DownloadProgressPayload }) => void),
  listen: vi.fn(),
  toastError: vi.fn(),
  toastSuccess: vi.fn(),
  setSettingValues: vi.fn(),
}));

vi.mock("@anlg/plugin-local-stt", () => ({
  commands: { getServerForModel: vi.fn() },
  events: {
    downloadProgressPayload: {
      listen: mocks.listen,
    },
  },
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  toast: { error: mocks.toastError, success: mocks.toastSuccess },
}));

vi.mock("~/settings/queries", () => ({
  setSettingValues: mocks.setSettingValues,
}));

vi.mock("~/shared/config", () => ({
  useConfigValues: () => ({
    current_stt_provider: "anarlog",
    current_stt_model: "cloud",
    current_llm_provider: "anarlog",
    current_llm_model: "default",
  }),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: { currentTab: null }) => unknown) =>
    selector({ currentTab: null }),
}));

vi.mock("~/stt/capabilities", () => ({
  isConfiguredSttModel: () => true,
  isOnDeviceSttModel: () => false,
}));

import { NotificationProvider, useNotifications } from "./notifications";

import { usePendingSttSelection } from "~/store/zustand/pending-stt-selection";

function DownloadStatus() {
  const { activeDownloads } = useNotifications();
  return (
    <div data-testid="download-status">
      {activeDownloads
        .map((download) =>
          download.isStarting ? "Starting" : `${download.progress}%`,
        )
        .join(",")}
    </div>
  );
}

describe("NotificationProvider", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    usePendingSttSelection.setState({ selection: null, queuedDownloads: [] });
    mocks.setSettingValues.mockResolvedValue(undefined);
    mocks.downloadHandler = null;
    mocks.listen.mockImplementation(async (handler) => {
      mocks.downloadHandler = handler;
      return vi.fn();
    });
  });

  it("surfaces an asynchronous model download failure", async () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <NotificationProvider>
          <div />
        </NotificationProvider>
      </QueryClientProvider>,
    );

    await waitFor(() => {
      expect(mocks.downloadHandler).not.toBeNull();
    });

    usePendingSttSelection.setState({
      selection: { provider: "soniqo", model: "soniqo-parakeet-batch" },
    });

    act(() => {
      mocks.downloadHandler?.({
        payload: {
          model: "soniqo-parakeet-batch",
          status: { failed: "download server rejected the model" },
        },
      });
    });

    expect(mocks.toastError).toHaveBeenCalledWith(
      "Couldn’t download Soniqo Parakeet Batch",
      { description: "download server rejected the model" },
    );
    expect(usePendingSttSelection.getState().selection).toBeNull();
  });

  it("selects the pending download only after completion and honors a newer choice", async () => {
    const queryClient = new QueryClient();
    render(
      <QueryClientProvider client={queryClient}>
        <NotificationProvider>
          <DownloadStatus />
        </NotificationProvider>
      </QueryClientProvider>,
    );
    act(() => {
      usePendingSttSelection.setState({
        selection: { provider: "soniqo", model: "soniqo-parakeet-batch" },
        queuedDownloads: ["soniqo-parakeet-batch"],
      });
    });
    expect(screen.getByTestId("download-status").textContent).toBe("Starting");
    act(() => {
      mocks.downloadHandler?.({
        payload: {
          model: "soniqo-parakeet-batch",
          status: { downloading: 50 },
        },
      });
    });
    expect(mocks.setSettingValues).not.toHaveBeenCalled();
    expect(screen.getByTestId("download-status").textContent).toBe("50%");
    expect(usePendingSttSelection.getState().selection?.model).toBe(
      "soniqo-parakeet-batch",
    );
    act(() => {
      mocks.downloadHandler?.({
        payload: { model: "soniqo-parakeet-batch", status: "completed" },
      });
    });
    await waitFor(() => {
      expect(usePendingSttSelection.getState().selection).toBeNull();
    });
    expect(mocks.setSettingValues).toHaveBeenCalledWith({
      current_stt_provider: "soniqo",
      current_stt_model: "soniqo-parakeet-batch",
    });
    expect(mocks.toastSuccess).toHaveBeenCalledWith("Model downloaded", {
      description: "Soniqo Parakeet Batch",
    });
    expect(screen.getByTestId("download-status").textContent).toBe("");

    mocks.setSettingValues.mockClear();
    usePendingSttSelection.setState({
      selection: { provider: "apple_speech", model: "apple-speech" },
    });
    act(() => {
      mocks.downloadHandler?.({
        payload: { model: "soniqo-parakeet-batch", status: "completed" },
      });
    });
    expect(mocks.setSettingValues).not.toHaveBeenCalled();
    expect(usePendingSttSelection.getState().selection?.model).toBe(
      "apple-speech",
    );
  });
});
