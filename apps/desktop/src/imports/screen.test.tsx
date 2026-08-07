import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  detectImportSources: vi.fn(),
}));

vi.mock("./detection", () => ({
  detectImportSources: mocks.detectImportSources,
}));

vi.mock("./queries", () => ({
  EMPTY_MEETING_IMPORT_HISTORY: [],
  importMeetingFiles: vi.fn(),
  useMeetingImportHistory: () => ({ data: [] }),
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

  it("shows only detected apps with recognizable prefix icons", async () => {
    mocks.detectImportSources.mockResolvedValue(
      MEETING_IMPORT_PROVIDERS.filter((provider) =>
        ["chatgpt-record", "granola", "slack-huddles", "zoom"].includes(
          provider.id,
        ),
      ).map((provider) => ({
        ...provider,
        installedAppId: `app.${provider.id}`,
        iconUrl: `data:image/png;base64,${provider.id}`,
      })),
    );

    const { container } = renderImports();

    expect(await screen.findByText("ChatGPT Record")).toBeTruthy();
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
      container.querySelectorAll('img[src^="data:image/png;base64,"]'),
    ).toHaveLength(4);
    expect(container.querySelector("iconify-icon")).toBeNull();
  });
});
