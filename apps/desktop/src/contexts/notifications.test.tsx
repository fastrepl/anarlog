import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  downloadHandler: null as
    | null
    | ((event: {
        payload: {
          model: "soniqo-parakeet-batch";
          status: { failed: string };
        };
      }) => void),
  listen: vi.fn(),
  toastError: vi.fn(),
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
  sonnerToast: { error: mocks.toastError },
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

import { NotificationProvider } from "./notifications";

describe("NotificationProvider", () => {
  beforeEach(() => {
    vi.clearAllMocks();
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
  });
});
