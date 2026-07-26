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
  updateAvailableListenMock,
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
  updateAvailableListenMock: vi.fn(),
  updateDownloadingListenMock: vi.fn(),
  updateDownloadProgressListenMock: vi.fn(),
  updateReadyListenMock: vi.fn(),
  updateDownloadFailedListenMock: vi.fn(),
  updatedListenMock: vi.fn(),
  eventHandlers: {
    updateAvailable: null as
      | null
      | ((event: { payload: { version: string } }) => void),
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
    updateAvailableEvent: {
      listen: updateAvailableListenMock,
    },
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
  useDesktopUpdateControl,
} from "./update-banner";

import { useDevtoolsOtaPreview } from "~/store/zustand/devtools-ota-preview";

const queryClients: QueryClient[] = [];

describe("SidebarTimelineUpdateButton", () => {
  beforeEach(() => {
    checkMock.mockReset();
    downloadMock.mockReset();
    installMock.mockReset();
    isDownloadedMock.mockReset();
    postinstallMock.mockReset();
    updateAvailableListenMock.mockReset();
    updateDownloadingListenMock.mockReset();
    updateDownloadProgressListenMock.mockReset();
    updateReadyListenMock.mockReset();
    updateDownloadFailedListenMock.mockReset();
    updatedListenMock.mockReset();

    eventHandlers.updateAvailable = null;
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

    updateAvailableListenMock.mockImplementation(async (handler) => {
      eventHandlers.updateAvailable = handler;
      return () => {};
    });
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

  it("downloads from the sidebar update button", async () => {
    checkMock.mockResolvedValue({ status: "ok", data: "1.0.34" });

    renderSidebarUpdateButton();

    const button = await screen.findByRole("button", {
      name: "Download update",
    });

    expect(button.className.split(" ")).toEqual(
      expect.arrayContaining(["h-7", "w-7", "min-h-7", "min-w-7", "p-0"]),
    );
    expect(button.className.split(" ")).toEqual(
      expect.arrayContaining(["bg-blue-500", "hover:bg-blue-600"]),
    );
    expect(button.className.split(" ")).not.toContain("bg-primary");

    fireEvent.click(button);

    await waitFor(() => expect(downloadMock).toHaveBeenCalledWith("1.0.34"));
  });

  it("shows download when an external update check reports an available version", async () => {
    renderSidebarUpdateButton();

    await waitFor(() =>
      expect(eventHandlers.updateAvailable).toBeTypeOf("function"),
    );

    act(() => {
      eventHandlers.updateAvailable?.({ payload: { version: "1.0.34" } });
    });

    fireEvent.click(screen.getByRole("button", { name: "Download update" }));

    await waitFor(() => expect(downloadMock).toHaveBeenCalledWith("1.0.34"));
  });

  it("clears stale available state after a successful check finds no update", async () => {
    renderSidebarUpdateButton();

    await waitFor(() =>
      expect(eventHandlers.updateAvailable).toBeTypeOf("function"),
    );

    act(() => {
      eventHandlers.updateAvailable?.({ payload: { version: "1.0.34" } });
    });

    expect(
      screen.getByRole("button", { name: "Download update" }),
    ).toBeTruthy();

    await act(async () => {
      await queryClients[queryClients.length - 1]?.refetchQueries({
        queryKey: ["updater2", "check"],
      });
    });

    await waitFor(() =>
      expect(
        screen.queryByRole("button", { name: "Download update" }),
      ).toBeNull(),
    );
  });

  it("keeps retry visible when a failed update is rechecked", async () => {
    renderSidebarUpdateButton();

    await waitFor(() =>
      expect(eventHandlers.updateDownloadFailed).toBeTypeOf("function"),
    );

    act(() => {
      eventHandlers.updateDownloadFailed?.({
        payload: { version: "1.0.34" },
      });
    });

    expect(screen.getByRole("button", { name: "Retry update" })).toBeTruthy();

    act(() => {
      eventHandlers.updateAvailable?.({ payload: { version: "1.0.34" } });
    });

    expect(screen.getByRole("button", { name: "Retry update" })).toBeTruthy();
    expect(
      screen.queryByRole("button", { name: "Download update" }),
    ).toBeNull();
  });

  it("shows restart when the checked update is already downloaded", async () => {
    checkMock.mockResolvedValue({ status: "ok", data: "1.0.34" });
    isDownloadedMock.mockResolvedValue({ status: "ok", data: true });

    renderSidebarUpdateButton();

    expect(
      await screen.findByRole("button", { name: "Restart to update" }),
    ).toBeTruthy();
    expect(
      screen.queryByRole("button", { name: "Download update" }),
    ).toBeNull();
  });

  it("keeps restart visible when an already-downloaded update also emits available", async () => {
    checkMock.mockResolvedValue({ status: "ok", data: "1.0.34" });
    isDownloadedMock.mockResolvedValue({ status: "ok", data: true });

    renderSidebarUpdateButton();

    expect(
      await screen.findByRole("button", { name: "Restart to update" }),
    ).toBeTruthy();

    act(() => {
      eventHandlers.updateAvailable?.({ payload: { version: "1.0.34" } });
    });

    expect(
      screen.getByRole("button", { name: "Restart to update" }),
    ).toBeTruthy();
    expect(
      screen.queryByRole("button", { name: "Download update" }),
    ).toBeNull();
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

  it("clears the button after the app reports it has updated", async () => {
    checkMock.mockResolvedValue({ status: "ok", data: "1.0.34" });

    renderSidebarUpdateButton();

    expect(
      await screen.findByRole("button", { name: "Download update" }),
    ).toBeTruthy();

    await waitFor(() => expect(eventHandlers.updated).toBeTypeOf("function"));

    act(() => {
      eventHandlers.updated?.({
        payload: { previous: "1.0.33", current: "1.0.34" },
      });
    });

    await waitFor(() =>
      expect(
        screen.queryByRole("button", { name: "Download update" }),
      ).toBeNull(),
    );
  });

  it("shows the devtools OTA preview state without a real updater result", async () => {
    useDevtoolsOtaPreview.getState().showPreview("available");

    renderSidebarUpdateButton();

    fireEvent.click(
      await screen.findByRole("button", { name: "Download update" }),
    );

    expect(
      await screen.findByRole("button", {
        name: "Downloading update, 58% complete",
      }),
    ).toBeTruthy();
    expect(downloadMock).not.toHaveBeenCalled();
  });
});

function renderSidebarUpdateButton() {
  return renderWithQueryClient(<SidebarUpdateButtonHarness />);
}

function SidebarUpdateButtonHarness() {
  const update = useDesktopUpdateControl();

  return <SidebarTimelineUpdateButton update={update} />;
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
