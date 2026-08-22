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
  connectNangoImport: vi.fn(),
  disconnectConnectedImport: vi.fn(),
  disconnectNangoImport: vi.fn(),
  signIn: vi.fn(),
  signedIn: true,
  connections: [] as Array<{
    connection_id: string;
    integration_id: string;
    status?: string | null;
  }>,
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({
    session: mocks.signedIn ? { user: { id: "user-1" } } : null,
    signIn: mocks.signIn,
    getHeaders: () =>
      mocks.signedIn ? { Authorization: "Bearer test" } : null,
  }),
}));

vi.mock("~/auth/useConnections", () => ({
  useConnections: () => ({
    data: mocks.connections,
    error: null,
    isPending: false,
  }),
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
  connectNangoImport: mocks.connectNangoImport,
  disconnectConnectedImport: mocks.disconnectConnectedImport,
  disconnectNangoImport: mocks.disconnectNangoImport,
  isDirectMeetingImport: (provider: { directImport?: string }) =>
    Boolean(provider.directImport),
  isNangoMeetingImport: (provider: { directImport?: string }) =>
    provider.directImport === "nango-oauth",
  isLocalConnectedImport: (provider: { directImport?: string }) =>
    provider.directImport === "mcp-oauth" || provider.directImport === "cli",
  nangoConnectionIsReady: (
    connection: { status?: string | null } | undefined,
  ) => Boolean(connection) && connection?.status !== "reconnect_required",
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
  nangoImportSyncQueryOptions: (
    provider: { id: string },
    connectionId: string | undefined,
    _headers: Record<string, string> | null,
    enabled: boolean,
  ) => ({
    queryKey: ["meeting-import", provider.id, "sync", connectionId],
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
  props: {
    compact?: boolean;
    onNoSourcesDetected?: () => void;
    secondaryAction?: ReactNode;
  } = {},
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
    mocks.signedIn = true;
    mocks.connections = [];
    mocks.cancelConnectedImport.mockResolvedValue(true);
    mocks.connectNangoImport.mockResolvedValue({
      connection_id: "zoom-1",
      integration_id: "zoom",
    });
    mocks.signIn.mockResolvedValue(undefined);
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
    ).toHaveLength(3);
    expect(screen.getAllByRole("button", { name: "Use files" })).toHaveLength(
      3,
    );
    expect(screen.queryByRole("menuitem", { name: "Use files" })).toBeNull();
    expect(
      screen.getAllByRole("button", { name: "Choose files" }),
    ).toHaveLength(2);
    expect(
      screen.getAllByText(/keep new meetings coming in while you switch/i),
    ).toHaveLength(3);
    expect(
      container.querySelectorAll('img[src^="data:image/png;base64,"]'),
    ).toHaveLength(4);
    expect(
      container.querySelector('img[src="/assets/zoom-icon.svg"]'),
    ).toBeTruthy();
    expect(container.querySelector("iconify-icon")).toBeNull();
  });

  it("uses official Meet and Zoom marks instead of a letter or wordmark", async () => {
    mocks.detectImportSources.mockResolvedValue([
      {
        ...MEETING_IMPORT_PROVIDERS.find(
          (provider) => provider.id === "google-meet",
        )!,
        installedAppId: "google-meet",
      },
      {
        ...MEETING_IMPORT_PROVIDERS.find((provider) => provider.id === "zoom")!,
        installedAppId: "us.zoom.xos",
        iconUrl: "data:image/png;base64,zoom-wordmark",
      },
      {
        ...MEETING_IMPORT_PROVIDERS.find(
          (provider) => provider.id === "granola",
        )!,
        installedAppId: "com.granola.app",
        iconUrl: "data:image/png;base64,granola",
      },
      {
        ...MEETING_IMPORT_PROVIDERS.find(
          (provider) => provider.id === "chatgpt-record",
        )!,
        installedAppId: "chatgpt-record",
      },
      {
        ...MEETING_IMPORT_PROVIDERS.find(
          (provider) => provider.id === "slack-huddles",
        )!,
        installedAppId: "slack-huddles",
      },
    ]);

    const { container } = renderImports();

    expect(await screen.findByText("Google Meet")).toBeTruthy();
    expect(
      container.querySelector('img[src="/assets/google-meet.svg"]'),
    ).toBeTruthy();
    expect(
      container.querySelector('img[src="/assets/zoom-icon.svg"]'),
    ).toBeTruthy();
    expect(
      container.querySelector('img[src="data:image/png;base64,granola"]')
        ?.className,
    ).not.toContain("scale-");
    expect(
      container.querySelector('img[src="/assets/model-icons/openai-logo.svg"]')
        ?.className,
    ).toContain("scale-[1.22]");
    expect(
      container.querySelector('img[src="/assets/slack-icon.svg"]')?.className,
    ).toContain("scale-[1.12]");
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

  it("prompts signed-out users to sign in before connecting", async () => {
    mocks.signedIn = false;
    mockDetected(["granola"]);

    renderImports();

    const signInButton = await screen.findByRole("button", {
      name: "Sign in to connect",
    });
    expect(screen.getByText("Connect & import")).toBeTruthy();
    expect(screen.getAllByText("Sign in to connect")).toHaveLength(2);
    expect(screen.getByRole("button", { name: "Use files" })).toBeTruthy();

    fireEvent.click(signInButton);

    await waitFor(() => {
      expect(mocks.signIn).toHaveBeenCalledOnce();
    });
    expect(mocks.connectConnectedImport).not.toHaveBeenCalled();
  });

  it("renders the same detected list in the compact onboarding layout", async () => {
    mockDetected(["granola", "slack-huddles"]);

    const { container } = renderImports({ compact: true });

    expect(await screen.findByText("Granola")).toBeTruthy();
    expect(screen.getByText("Slack Huddles")).toBeTruthy();
    expect(screen.queryByText("Circleback")).toBeNull();
    expect(
      screen.getAllByRole("button", { name: "Connect & import" }),
    ).toHaveLength(1);
    expect(
      screen.getAllByRole("button", { name: "Choose files" }),
    ).toHaveLength(1);

    const list = container.querySelector(".rounded-2xl");
    expect(list).toBeTruthy();
    expect(list?.className).toContain("overflow-hidden");
    expect(list?.className).not.toContain("overflow-y-auto");
    expect(list?.querySelector(".overflow-y-auto")).toBeTruthy();
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

  it("connects Zoom through Nango OAuth instead of file-only import", async () => {
    mockDetected(["zoom"]);

    renderImports();

    fireEvent.click(
      await screen.findByRole("button", { name: "Connect & import" }),
    );

    await waitFor(() => {
      expect(mocks.connectNangoImport).toHaveBeenCalledOnce();
    });
    expect(mocks.connectConnectedImport).not.toHaveBeenCalled();
    expect(
      screen.getByText(/keep new meetings coming in while you switch/i),
    ).toBeTruthy();
    expect(
      screen.queryByText(/Direct connection is not available yet/i),
    ).toBeNull();
  });

  it("connects Plaud by running the local CLI instead of file-only import", async () => {
    mockDetected(["plaud"]);
    mocks.connectConnectedImport.mockResolvedValue({
      providerId: "plaud",
      clientId: "ada@example.com",
      tokenJson: "{}",
    });

    renderImports();

    fireEvent.click(
      await screen.findByRole("button", { name: "Connect & import" }),
    );

    await waitFor(() => {
      expect(mocks.connectConnectedImport).toHaveBeenCalledOnce();
    });
    expect(mocks.connectNangoImport).not.toHaveBeenCalled();
    expect(
      screen.getByText(
        /Connected · New meetings are imported automatically while Anarlog is running/i,
      ),
    ).toBeTruthy();
    expect(
      screen.queryByText(/Direct connection is not available yet/i),
    ).toBeNull();
  });

  it("connects Pocket through MCP OAuth instead of file-only import", async () => {
    mockDetected(["pocket"]);
    mocks.connectConnectedImport.mockResolvedValue({
      providerId: "pocket",
      clientId: "pocket-client",
      tokenJson: "{}",
    });

    renderImports();

    fireEvent.click(
      await screen.findByRole("button", { name: "Connect & import" }),
    );

    await waitFor(() => {
      expect(mocks.connectConnectedImport).toHaveBeenCalledOnce();
    });
    expect(mocks.connectNangoImport).not.toHaveBeenCalled();
    expect(
      screen.getByText(
        /Connected · New meetings are imported automatically while Anarlog is running/i,
      ),
    ).toBeTruthy();
  });

  it("shows the empty state when nothing is detected", async () => {
    mockDetected([]);

    renderImports();

    expect(await screen.findByText("No apps found.")).toBeTruthy();
  });

  it("reports when detection finishes without finding any apps", async () => {
    const onNoSourcesDetected = vi.fn();
    mockDetected([]);

    renderImports({ onNoSourcesDetected });

    await waitFor(() => {
      expect(onNoSourcesDetected).toHaveBeenCalledOnce();
    });
  });

  it("does not report an empty result while detection is pending", async () => {
    const onNoSourcesDetected = vi.fn();
    mocks.detectImportSources.mockReturnValue(new Promise(() => {}));

    renderImports({ onNoSourcesDetected });

    expect(
      await screen.findByText("Checking installed meeting assistants…"),
    ).toBeTruthy();
    expect(onNoSourcesDetected).not.toHaveBeenCalled();
  });

  it("does not report an empty result when detection fails", async () => {
    const onNoSourcesDetected = vi.fn();
    mocks.detectImportSources.mockRejectedValue(new Error("Detection failed"));

    renderImports({ onNoSourcesDetected });

    expect(await screen.findByText("Detection failed")).toBeTruthy();
    expect(onNoSourcesDetected).not.toHaveBeenCalled();
  });
});
