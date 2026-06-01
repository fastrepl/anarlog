import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { type ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const {
  checkMock,
  downloadMock,
  installMock,
  isDownloadedMock,
  postinstallMock,
  updateDownloadingListenMock,
  updateDownloadProgressListenMock,
  updateReadyListenMock,
  updateDownloadFailedListenMock,
  updatedListenMock,
  eventHandlers,
} = vi.hoisted(() => ({
  checkMock: vi.fn(),
  downloadMock: vi.fn(),
  installMock: vi.fn(),
  isDownloadedMock: vi.fn(),
  postinstallMock: vi.fn(),
  updateDownloadingListenMock: vi.fn(),
  updateDownloadProgressListenMock: vi.fn(),
  updateReadyListenMock: vi.fn(),
  updateDownloadFailedListenMock: vi.fn(),
  updatedListenMock: vi.fn(),
  eventHandlers: {
    updateDownloading: null as
      | null
      | ((event: { payload: { version: string } }) => void),
    updateDownloadProgress: null as
      | null
      | ((event: {
          payload: {
            version: string;
            chunk_length: number;
            content_length: number | null;
          };
        }) => void),
    updateReady: null as
      | null
      | ((event: { payload: { version: string } }) => void),
    updateDownloadFailed: null as
      | null
      | ((event: { payload: { version: string } }) => void),
    updated: null as
      | null
      | ((event: {
          payload: { previous: string | null; current: string };
        }) => void),
  },
}));

vi.mock("@hypr/plugin-updater2", () => ({
  commands: {
    check: checkMock,
    download: downloadMock,
    install: installMock,
    isDownloaded: isDownloadedMock,
    postinstall: postinstallMock,
  },
  events: {
    updateDownloadingEvent: {
      listen: updateDownloadingListenMock,
    },
    updateDownloadProgressEvent: {
      listen: updateDownloadProgressListenMock,
    },
    updateReadyEvent: {
      listen: updateReadyListenMock,
    },
    updateDownloadFailedEvent: {
      listen: updateDownloadFailedListenMock,
    },
    updatedEvent: {
      listen: updatedListenMock,
    },
  },
}));

import {
  SidebarTimelineUpdateButton,
  TimelineUpdateBanner,
  useDesktopUpdateControl,
} from "./update-banner";

import { useDevtoolsOtaPreview } from "~/store/zustand/devtools-ota-preview";

const queryClients: QueryClient[] = [];

describe("TimelineUpdateBanner", () => {
  beforeEach(() => {
    checkMock.mockReset();
    downloadMock.mockReset();
    installMock.mockReset();
    isDownloadedMock.mockReset();
    postinstallMock.mockReset();
    updateDownloadingListenMock.mockReset();
    updateDownloadProgressListenMock.mockReset();
    updateReadyListenMock.mockReset();
    updateDownloadFailedListenMock.mockReset();
    updatedListenMock.mockReset();

    eventHandlers.updateDownloading = null;
    eventHandlers.updateDownloadProgress = null;
    eventHandlers.updateReady = null;
    eventHandlers.updateDownloadFailed = null;
    eventHandlers.updated = null;

    checkMock.mockResolvedValue({ status: "ok", data: null });
    downloadMock.mockResolvedValue({ status: "ok", data: null });
    installMock.mockResolvedValue({
      status: "ok",
      data: { kind: "relaunch_current" },
    });
    isDownloadedMock.mockResolvedValue({ status: "ok", data: false });
    postinstallMock.mockResolvedValue({ status: "ok", data: null });

    updateDownloadingListenMock.mockImplementation(async (handler) => {
      eventHandlers.updateDownloading = handler;
      return () => {};
    });
    updateDownloadProgressListenMock.mockImplementation(async (handler) => {
      eventHandlers.updateDownloadProgress = handler;
      return () => {};
    });
    updateReadyListenMock.mockImplementation(async (handler) => {
      eventHandlers.updateReady = handler;
      return () => {};
    });
    updateDownloadFailedListenMock.mockImplementation(async (handler) => {
      eventHandlers.updateDownloadFailed = handler;
      return () => {};
    });
    updatedListenMock.mockImplementation(async (handler) => {
      eventHandlers.updated = handler;
      return () => {};
    });

    useDevtoolsOtaPreview.getState().clearPreview();
  });

  afterEach(() => {
    cleanup();
    queryClients.forEach((queryClient) => queryClient.clear());
    queryClients.length = 0;
    useDevtoolsOtaPreview.getState().clearPreview();
  });

  it("shows an available update from the updater check", async () => {
    checkMock.mockResolvedValue({ status: "ok", data: "1.0.34" });

    renderBanner();

    expect(await screen.findByText("New version available")).toBeTruthy();
    expect(screen.getByRole("button", { name: /Download/ })).toBeTruthy();
  });

  it("downloads and restarts from the banner", async () => {
    checkMock.mockResolvedValue({ status: "ok", data: "1.0.34" });

    renderBanner();

    fireEvent.click(await screen.findByRole("button", { name: /Download/ }));

    await waitFor(() => expect(downloadMock).toHaveBeenCalledWith("1.0.34"));

    await waitFor(() =>
      expect(eventHandlers.updateReady).toBeTypeOf("function"),
    );

    act(() => {
      eventHandlers.updateReady?.({ payload: { version: "1.0.34" } });
    });

    fireEvent.click(screen.getByRole("button", { name: /Restart/ }));

    await waitFor(() => {
      expect(installMock).toHaveBeenCalledWith("1.0.34");
      expect(postinstallMock).toHaveBeenCalledWith({
        kind: "relaunch_current",
      });
    });
  });

  it("shows restart when the checked update is already downloaded", async () => {
    checkMock.mockResolvedValue({ status: "ok", data: "1.0.34" });
    isDownloadedMock.mockResolvedValue({ status: "ok", data: true });

    renderBanner();

    expect(await screen.findByText("Update ready")).toBeTruthy();
    expect(screen.getByRole("button", { name: /Restart/ })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Download/ })).toBeNull();
  });

  it("shows a failed state when postinstall returns an error result", async () => {
    checkMock.mockResolvedValue({ status: "ok", data: "1.0.34" });
    postinstallMock.mockResolvedValue({
      status: "error",
      error: "Restart failed",
    });

    renderBanner();

    fireEvent.click(await screen.findByRole("button", { name: /Download/ }));

    await waitFor(() =>
      expect(screen.getByRole("button", { name: /Restart/ })).toBeTruthy(),
    );

    fireEvent.click(screen.getByRole("button", { name: /Restart/ }));

    expect(await screen.findByText("Update failed")).toBeTruthy();
  });

  it("shows download progress from updater events", async () => {
    renderBanner();

    await waitFor(() =>
      expect(eventHandlers.updateDownloadProgress).toBeTypeOf("function"),
    );

    act(() => {
      eventHandlers.updateDownloading?.({ payload: { version: "1.0.34" } });
      eventHandlers.updateDownloadProgress?.({
        payload: {
          version: "1.0.34",
          chunk_length: 25,
          content_length: 100,
        },
      });
      eventHandlers.updateDownloadProgress?.({
        payload: {
          version: "1.0.34",
          chunk_length: 25,
          content_length: 100,
        },
      });
    });

    expect(screen.getByText("New version available")).toBeTruthy();
    expect(screen.getByText("50%")).toBeTruthy();
  });

  it("clears the banner after the app reports it has updated", async () => {
    checkMock.mockResolvedValue({ status: "ok", data: "1.0.34" });

    renderBanner();

    expect(await screen.findByText("New version available")).toBeTruthy();

    await waitFor(() => expect(eventHandlers.updated).toBeTypeOf("function"));

    act(() => {
      eventHandlers.updated?.({
        payload: { previous: "1.0.33", current: "1.0.34" },
      });
    });

    await waitFor(() =>
      expect(screen.queryByText("New version available")).toBeNull(),
    );
  });

  it("downloads from the sidebar update button", async () => {
    checkMock.mockResolvedValue({ status: "ok", data: "1.0.34" });

    renderSidebarUpdateButton();

    fireEvent.click(
      await screen.findByRole("button", { name: "Download update" }),
    );

    await waitFor(() => expect(downloadMock).toHaveBeenCalledWith("1.0.34"));
  });

  it("shows sidebar circular progress while downloading", async () => {
    renderSidebarUpdateButton();

    await waitFor(() =>
      expect(eventHandlers.updateDownloadProgress).toBeTypeOf("function"),
    );

    act(() => {
      eventHandlers.updateDownloading?.({ payload: { version: "1.0.34" } });
      eventHandlers.updateDownloadProgress?.({
        payload: {
          version: "1.0.34",
          chunk_length: 50,
          content_length: 100,
        },
      });
    });

    const button = screen.getByRole("button", {
      name: "Downloading update, 50% complete",
    });

    expect(button.hasAttribute("disabled")).toBe(true);
    expect(button.querySelector(".lucide-download")).toBeNull();
    expect(button.querySelector(".lucide-loader-circle")).toBeNull();
  });

  it("restarts from the sidebar update button when ready", async () => {
    renderSidebarUpdateButton();

    await waitFor(() =>
      expect(eventHandlers.updateReady).toBeTypeOf("function"),
    );

    act(() => {
      eventHandlers.updateReady?.({ payload: { version: "1.0.34" } });
    });

    fireEvent.click(screen.getByRole("button", { name: "Restart to update" }));

    await waitFor(() => {
      expect(installMock).toHaveBeenCalledWith("1.0.34");
      expect(postinstallMock).toHaveBeenCalledWith({
        kind: "relaunch_current",
      });
    });
  });

  it("shows the devtools OTA preview state without a real updater result", async () => {
    useDevtoolsOtaPreview.getState().showPreview("available");

    renderBanner();

    expect(await screen.findByText("New version available")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: /Download/ }));

    expect(
      await screen.findByLabelText("Downloading update, 58% complete"),
    ).toBeTruthy();
    expect(downloadMock).not.toHaveBeenCalled();
  });
});

function renderBanner() {
  return renderWithQueryClient(<TimelineUpdateBannerHarness />);
}

function renderSidebarUpdateButton() {
  return renderWithQueryClient(<SidebarUpdateButtonHarness />);
}

function SidebarUpdateButtonHarness() {
  const update = useDesktopUpdateControl();

  return <SidebarTimelineUpdateButton update={update} />;
}

function TimelineUpdateBannerHarness() {
  const update = useDesktopUpdateControl();

  return <TimelineUpdateBanner update={update} />;
}

function renderWithQueryClient(ui: ReactNode) {
  const queryClient = new QueryClient({
    defaultOptions: {
      mutations: { retry: false },
      queries: { retry: false },
    },
  });
  queryClients.push(queryClient);

  return render(
    <QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>,
  );
}
