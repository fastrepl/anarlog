import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  detectImportSources: vi.fn(),
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

function renderImports() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <MeetingImportScreen />
    </QueryClientProvider>,
  );
}

describe("MeetingImportScreen", () => {
  afterEach(cleanup);

  it("shows direct connections plus detected file sources with native icons", async () => {
    mocks.detectImportSources.mockResolvedValue(
      MEETING_IMPORT_PROVIDERS.filter((provider) =>
        [
          "chatgpt-record",
          "circleback",
          "granola",
          "slack-huddles",
          "zoom",
        ].includes(provider.id),
      ).map((provider) => ({
        ...provider,
        installedAppId: `app.${provider.id}`,
        iconUrl: `data:image/png;base64,${provider.id}`,
      })),
    );

    const { container } = renderImports();

    expect(await screen.findByText("ChatGPT Record")).toBeTruthy();
    expect(screen.getByText("Circleback")).toBeTruthy();
    expect(screen.getByText("Granola")).toBeTruthy();
    expect(screen.getByText("Slack Huddles")).toBeTruthy();
    expect(screen.getByText("Zoom")).toBeTruthy();
    expect(screen.queryByText("Avoma")).toBeNull();
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(screen.queryByText("Detected")).toBeNull();
    expect(screen.queryByText("Export")).toBeNull();
    expect(screen.queryByText("OAuth")).toBeNull();
    expect(screen.queryByText("Export help")).toBeNull();
    expect(
      screen.getAllByRole("button", { name: "Connect & import" }),
    ).toHaveLength(8);
    expect(screen.getAllByRole("button", { name: "Use files" })).toHaveLength(
      8,
    );
    expect(
      screen.getAllByRole("button", { name: "Choose files" }),
    ).toHaveLength(3);
    expect(
      screen.getAllByText(/keep new meetings coming in while you switch/i),
    ).toHaveLength(8);
    expect(
      container.querySelectorAll('img[src^="data:image/png;base64,"]'),
    ).toHaveLength(5);
    expect(container.querySelector("iconify-icon")).toBeNull();
  });
});
