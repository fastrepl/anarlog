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

vi.mock("@anlg/plugin-updater2", () => ({
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

import { resolveUpdateState, useDesktopUpdateControl } from "./update-banner";

import { useDevtoolsOtaPreview } from "~/store/zustand/devtools-ota-preview";

const queryClients: QueryClient[] = [];

describe("useDesktopUpdateControl", () => {
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

  it("downloads the update reported by the update check", async () => {
    checkMock.mockResolvedValue({ status: "ok", data: "1.0.34" });

    renderUpdateControl();

    await waitFor(() =>
      expect(screen.getByTestId("status").textContent).toBe("available"),
    );

    fireEvent.click(screen.getByRole("button", { name: "download" }));

    await waitFor(() => expect(downloadMock).toHaveBeenCalledWith("1.0.34"));
  });

  it("reports available when an external update check emits an event", async () => {
    renderUpdateControl();

    await waitFor(() =>
      expect(eventHandlers.updateAvailable).toBeTypeOf("function"),
    );

    act(() => {
      eventHandlers.updateAvailable?.({ payload: { version: "1.0.34" } });
    });

    expect(screen.getByTestId("status").textContent).toBe("available");

    fireEvent.click(screen.getByRole("button", { name: "download" }));

    await waitFor(() => expect(downloadMock).toHaveBeenCalledWith("1.0.34"));
  });

  it("clears stale available state after a successful check finds no update", async () => {
    renderUpdateControl();

    await waitFor(() =>
      expect(eventHandlers.updateAvailable).toBeTypeOf("function"),
    );

    act(() => {
      eventHandlers.updateAvailable?.({ payload: { version: "1.0.34" } });
    });

    expect(screen.getByTestId("status").textContent).toBe("available");

    await act(async () => {
      await queryClients[queryClients.length - 1]?.refetchQueries({
        queryKey: ["updater2", "check"],
      });
    });

    await waitFor(() =>
      expect(screen.getByTestId("status").textContent).toBe("none"),
    );
  });

  it("keeps failed state when a failed update is rechecked", async () => {
    renderUpdateControl();

    await waitFor(() =>
      expect(eventHandlers.updateDownloadFailed).toBeTypeOf("function"),
    );

    act(() => {
      eventHandlers.updateDownloadFailed?.({
        payload: { version: "1.0.34" },
      });
    });

    expect(screen.getByTestId("status").textContent).toBe("failed");

    act(() => {
      eventHandlers.updateAvailable?.({ payload: { version: "1.0.34" } });
    });

    expect(screen.getByTestId("status").textContent).toBe("failed");
  });

  it("reports ready when the checked update is already downloaded", async () => {
    checkMock.mockResolvedValue({ status: "ok", data: "1.0.34" });
    isDownloadedMock.mockResolvedValue({ status: "ok", data: true });

    renderUpdateControl();

    await waitFor(() =>
      expect(screen.getByTestId("status").textContent).toBe("ready"),
    );
  });

  it("keeps ready state when an already-downloaded update also emits available", async () => {
    checkMock.mockResolvedValue({ status: "ok", data: "1.0.34" });
    isDownloadedMock.mockResolvedValue({ status: "ok", data: true });

    renderUpdateControl();

    await waitFor(() =>
      expect(screen.getByTestId("status").textContent).toBe("ready"),
    );

    act(() => {
      eventHandlers.updateAvailable?.({ payload: { version: "1.0.34" } });
    });

    expect(screen.getByTestId("status").textContent).toBe("ready");
  });

  it("tracks download progress from updater events", async () => {
    renderUpdateControl();

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

    expect(screen.getByTestId("status").textContent).toBe("downloading");
    expect(screen.getByTestId("progress").textContent).toBe("0.5");
  });

  it("installs the update when ready", async () => {
    renderUpdateControl();

    await waitFor(() =>
      expect(eventHandlers.updateReady).toBeTypeOf("function"),
    );

    act(() => {
      eventHandlers.updateReady?.({ payload: { version: "1.0.34" } });
    });

    expect(screen.getByTestId("status").textContent).toBe("ready");

    fireEvent.click(screen.getByRole("button", { name: "install" }));

    await waitFor(() => {
      expect(installMock).toHaveBeenCalledWith("1.0.34");
      expect(postinstallMock).toHaveBeenCalledWith({
        kind: "relaunch_current",
      });
    });
  });

  it("clears the update after the app reports it has updated", async () => {
    checkMock.mockResolvedValue({ status: "ok", data: "1.0.34" });

    renderUpdateControl();

    await waitFor(() =>
      expect(screen.getByTestId("status").textContent).toBe("available"),
    );

    await waitFor(() => expect(eventHandlers.updated).toBeTypeOf("function"));

    act(() => {
      eventHandlers.updated?.({
        payload: { previous: "1.0.33", current: "1.0.34" },
      });
    });

    await waitFor(() =>
      expect(screen.getByTestId("status").textContent).toBe("none"),
    );
  });

  it("shows the devtools OTA preview state without a real updater result", async () => {
    useDevtoolsOtaPreview.getState().showPreview("available");

    renderUpdateControl();

    expect(screen.getByTestId("status").textContent).toBe("available");

    fireEvent.click(screen.getByRole("button", { name: "download" }));

    await waitFor(() =>
      expect(screen.getByTestId("status").textContent).toBe("downloading"),
    );
    expect(screen.getByTestId("progress").textContent).toBe("0.58");
    expect(downloadMock).not.toHaveBeenCalled();
  });

  it("shows a bounded download when progress arrives before availability", async () => {
    renderUpdateControl();

    await waitFor(() =>
      expect(eventHandlers.updateDownloadProgress).toBeTypeOf("function"),
    );

    act(() => {
      eventHandlers.updateDownloadProgress?.({
        payload: {
          version: "1.0.34",
          chunk_length: 25,
          content_length: 100,
        },
      });
    });

    expect(screen.getByTestId("status").textContent).toBe("downloading");
    expect(screen.getByTestId("progress").textContent).toBe("0.25");

    act(() => {
      eventHandlers.updateDownloadProgress?.({
        payload: {
          version: "1.0.34",
          chunk_length: 75,
          content_length: 100,
        },
      });
    });

    expect(screen.getByTestId("progress").textContent).toBe("1");
  });
});

function renderUpdateControl() {
  return renderWithQueryClient(<UpdateControlProbe />);
}

function UpdateControlProbe() {
  const update = useDesktopUpdateControl();

  return (
    <div>
      <span data-testid="status">{update.status ?? "none"}</span>
      <span data-testid="version">{update.version ?? "none"}</span>
      <span data-testid="progress">
        {update.progress === null ? "none" : String(update.progress)}
      </span>
      <button type="button" onClick={update.downloadUpdate}>
        download
      </button>
      <button type="button" onClick={update.installUpdate}>
        install
      </button>
    </div>
  );
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

describe("resolveUpdateState", () => {
  it("projects the check result when no event arrived", () => {
    expect(resolveUpdateState(null, null, null)).toEqual({ kind: "none" });
    expect(
      resolveUpdateState(null, { version: "1.0.34", ready: false }, null),
    ).toEqual({ kind: "available", version: "1.0.34" });
    expect(
      resolveUpdateState(null, { version: "1.0.34", ready: true }, null),
    ).toEqual({ kind: "ready", version: "1.0.34" });
  });

  it("upgrades an available event to ready when the check says downloaded", () => {
    expect(
      resolveUpdateState(
        { kind: "available", version: "1.0.34" },
        { version: "1.0.34", ready: true },
        null,
      ),
    ).toEqual({ kind: "ready", version: "1.0.34" });
    expect(
      resolveUpdateState(
        { kind: "available", version: "1.0.34" },
        { version: "1.0.35", ready: true },
        null,
      ),
    ).toEqual({ kind: "available", version: "1.0.34" });
  });

  it("keeps event precedence over the check for active and failed states", () => {
    const downloading = {
      kind: "downloading" as const,
      version: "1.0.34",
      downloadedBytes: 10,
      contentLength: 100,
    };
    expect(
      resolveUpdateState(downloading, { version: "1.0.35", ready: true }, null),
    ).toEqual(downloading);

    const failed = {
      kind: "failed" as const,
      version: "1.0.34",
      errorMessage: "boom",
    };
    expect(
      resolveUpdateState(failed, { version: "1.0.34", ready: true }, null),
    ).toEqual(failed);
  });

  it("suppresses acknowledged versions from the check but not events", () => {
    expect(
      resolveUpdateState(null, { version: "1.0.34", ready: true }, "1.0.34"),
    ).toEqual({ kind: "none" });
    expect(
      resolveUpdateState(
        { kind: "available", version: "1.0.34" },
        { version: "1.0.34", ready: true },
        "1.0.34",
      ),
    ).toEqual({ kind: "available", version: "1.0.34" });
  });
});
