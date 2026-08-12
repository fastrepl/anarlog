import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  detectImportSources: vi.fn(),
  cancelConnectedImport: vi.fn(),
  connectConnectedImport: vi.fn(),
  disconnectConnectedImport: vi.fn(),
}));

vi.mock("./detection", () => ({
  detectImportSources: mocks.detectImportSources,
}));

vi.mock("./queries", () => ({
  EMPTY_MEETING_IMPORT_HISTORY: [],
  importConnectedMeetings: vi.fn(),
  importMeetingFiles: vi.fn(),
  useMeetingImportHistory: () => ({ data: [] }),
}));

vi.mock("./connected-import", () => ({
  cancelConnectedImport: mocks.cancelConnectedImport,
  connectConnectedImport: mocks.connectConnectedImport,
  disconnectConnectedImport: mocks.disconnectConnectedImport,
  connectedImportCredentialsQueryKey: (providerId: string) => [
    "meeting-import",
    providerId,
    "credentials",
  ],
  connectedImportSyncQueryKey: (providerId: string) => [
    "meeting-import",
    providerId,
    "sync",
  ],
  connectedImportCredentialsQueryOptions: (providerId: string) => ({
    queryKey: ["meeting-import", providerId, "credentials"],
    queryFn: async () => null,
    staleTime: Infinity,
  }),
  connectedImportSyncQueryOptions: (
    provider: { id: string },
    enabled: boolean,
  ) => ({
    queryKey: ["meeting-import", provider.id, "sync"],
    queryFn: async () => ({
      result: {
        discovered: 0,
        imported: 0,
        matched: 0,
        conflicts: 0,
        errors: 0,
      },
      warnings: [],
    }),
    enabled,
    retry: false,
  }),
}));

vi.mock("./termination-pause", () => ({
  pauseCompetingApplicationTermination: vi.fn(),
}));

import { MEETING_IMPORT_PROVIDERS } from "./providers";
import { MeetingImportScreen } from "./screen";

function renderImports(
  props: { compact?: boolean; secondaryAction?: ReactNode } = {},
) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <MeetingImportScreen {...props} />
    </QueryClientProvider>,
  );
}

function mockDetected(ids: string[]) {
  mocks.detectImportSources.mockResolvedValue(
    MEETING_IMPORT_PROVIDERS.filter((provider) =>
      ids.includes(provider.id),
    ).map((provider) => ({
      ...provider,
      installedAppId: `app.${provider.id}`,
      iconUrl: `data:image/png;base64,${provider.id}`,
    })),
  );
}

describe("MeetingImportScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.cancelConnectedImport.mockResolvedValue(true);
  });

  afterEach(cleanup);

  it("lists only detected apps with native icons", async () => {
    mockDetected([
      "chatgpt-record",
      "circleback",
      "granola",
      "slack-huddles",
      "zoom",
    ]);

    const { container } = renderImports();

    expect(await screen.findByText("ChatGPT Record")).toBeTruthy();
    expect(screen.getByText("Circleback")).toBeTruthy();
    expect(screen.getByText("Granola")).toBeTruthy();
    expect(screen.getByText("Slack Huddles")).toBeTruthy();
    expect(screen.getByText("Zoom")).toBeTruthy();
    expect(screen.queryByText("Avoma")).toBeNull();
    expect(screen.queryByText("Fireflies.ai")).toBeNull();
    expect(screen.queryByText("Krisp")).toBeNull();
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(screen.queryByText("Detected")).toBeNull();
    expect(screen.queryByText("Export")).toBeNull();
    expect(screen.queryByText("OAuth")).toBeNull();
    expect(screen.queryByText("Export help")).toBeNull();
    expect(
      screen.getAllByRole("button", { name: "Connect & import" }),
    ).toHaveLength(2);
    expect(screen.getAllByRole("button", { name: "Use files" })).toHaveLength(
      2,
    );
    expect(screen.queryByRole("menuitem", { name: "Use files" })).toBeNull();
    expect(
      screen.getAllByRole("button", { name: "Choose files" }),
    ).toHaveLength(3);
    expect(
      screen.getAllByText(/keep new meetings coming in while you switch/i),
    ).toHaveLength(2);
    expect(
      container.querySelectorAll('img[src^="data:image/png;base64,"]'),
    ).toHaveLength(5);
    expect(container.querySelector("iconify-icon")).toBeNull();
  });

  it("offers file import from the connected provider menu", async () => {
    mockDetected(["granola"]);

    renderImports();

    const trigger = await screen.findByRole("button", {
      name: "Use files",
    });
    fireEvent.pointerDown(trigger);

    expect(
      await screen.findByRole("menuitem", { name: "Use files" }),
    ).toBeTruthy();
  });

  it("renders the same detected list in the compact onboarding layout", async () => {
    mockDetected(["granola", "slack-huddles"]);

    renderImports({ compact: true });

    expect(await screen.findByText("Granola")).toBeTruthy();
    expect(screen.getByText("Slack Huddles")).toBeTruthy();
    expect(screen.queryByText("Circleback")).toBeNull();
    expect(
      screen.getAllByRole("button", { name: "Connect & import" }),
    ).toHaveLength(1);
    expect(
      screen.getAllByRole("button", { name: "Choose files" }),
    ).toHaveLength(1);
  });

  it("renders the secondary action even before anything is imported", async () => {
    mockDetected(["granola"]);

    renderImports({
      compact: true,
      secondaryAction: <button type="button">Skip for now</button>,
    });

    expect(
      await screen.findByRole("button", { name: "Skip for now" }),
    ).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Continue" })).toBeNull();
  });

  it("lets the user cancel an abandoned browser connection and retry", async () => {
    mockDetected(["granola"]);
    mocks.connectConnectedImport.mockImplementation(
      (_provider: unknown, signal: AbortSignal) =>
        new Promise((_, reject) => {
          signal.addEventListener("abort", () => reject(signal.reason), {
            once: true,
          });
        }),
    );

    renderImports();

    fireEvent.click(
      await screen.findByRole("button", { name: "Connect & import" }),
    );
    const cancelButton = await screen.findByRole("button", { name: "Cancel" });
    fireEvent.click(cancelButton);

    await waitFor(() => {
      expect(mocks.cancelConnectedImport.mock.calls[0]?.[0]).toBe("granola");
      expect(
        screen
          .getByRole("button", { name: "Connect & import" })
          .hasAttribute("disabled"),
      ).toBe(false);
    });

    fireEvent.click(screen.getByRole("button", { name: "Connect & import" }));
    await waitFor(() => {
      expect(mocks.connectConnectedImport).toHaveBeenCalledTimes(2);
    });
  });

  it("shows the empty state when nothing is detected", async () => {
    mockDetected([]);

    renderImports();

    expect(await screen.findByText("No apps found.")).toBeTruthy();
  });
});
