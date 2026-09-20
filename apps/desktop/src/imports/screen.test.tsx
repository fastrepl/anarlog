import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
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
  sync: vi.fn(),
  selectFiles: vi.fn(),
  readTextFiles: vi.fn(),
  importMeetingFiles: vi.fn(),
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

vi.mock("~/auth/useConnections", async () => {
  const { useQuery } = await import("@tanstack/react-query");
  return {
    useConnections: () =>
      useQuery({
        queryKey: ["integration-status", "user-1"],
        queryFn: async () => mocks.connections,
        initialData: mocks.connections,
      }),
  };
});

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: mocks.selectFiles }));
vi.mock("@anlg/plugin-importer", () => ({
  commands: { readTextFiles: mocks.readTextFiles },
}));

vi.mock("./detection", () => ({
  detectImportSources: mocks.detectImportSources,
}));

vi.mock("./queries", () => ({
  EMPTY_MEETING_IMPORT_HISTORY: [],
  importConnectedMeetings: vi.fn(),
  importMeetingFiles: mocks.importMeetingFiles,
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
    queryFn: () => mocks.sync(provider.id),
    enabled,
    retry: false,
  }),
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
    mocks.selectFiles.mockResolvedValue(["export.json"]);
    mocks.readTextFiles.mockResolvedValue({ status: "ok", data: [] });
    mocks.signedIn = true;
    mocks.connections = [];
    mocks.sync.mockResolvedValue({
      result: {
        discovered: 0,
        imported: 0,
        matched: 0,
        conflicts: 0,
        errors: 0,
      },
      warnings: [],
    });
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
    expect(screen.getAllByRole("button", { name: "Connect" })).toHaveLength(3);
    expect(
      screen.getAllByRole("button", { name: "Connect" })[0]?.className,
    ).toContain("hover:bg-primary-foreground/10");
    expect(
      screen
        .getAllByRole("button", { name: "Connect" })[0]
        ?.closest('[role="group"]')?.parentElement?.className,
    ).toContain("focus-within:ring-[3px]");
    expect(screen.getAllByRole("button", { name: "Use files" })).toHaveLength(
      3,
    );
    expect(
      screen.getAllByRole("button", { name: "Use files" })[0]?.className,
    ).toContain("hover:bg-primary-foreground/10");
    expect(screen.queryByRole("menuitem", { name: "Use files" })).toBeNull();
    expect(
      screen.getAllByRole("button", { name: "Choose files" }),
    ).toHaveLength(2);
    expect(
      screen.queryByText(/keep new meetings coming in while you switch/i),
    ).toBeNull();
    expect(
      screen.queryByText("Choose files exported from this app."),
    ).toBeNull();
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
    expect(screen.getByText("Connect")).toBeTruthy();
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
    expect(screen.getAllByRole("button", { name: "Connect" })).toHaveLength(1);
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

    fireEvent.click(await screen.findByRole("button", { name: "Connect" }));
    const cancelButton = await screen.findByRole("button", { name: "Cancel" });
    fireEvent.click(cancelButton);

    await waitFor(() => {
      expect(mocks.cancelConnectedImport.mock.calls[0]?.[0]).toBe("granola");
      expect(
        screen
          .getByRole("button", { name: "Connect" })
          .hasAttribute("disabled"),
      ).toBe(false);
    });

    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    await waitFor(() => {
      expect(mocks.connectConnectedImport).toHaveBeenCalledTimes(2);
    });
  });

  it("connects Zoom through Nango OAuth instead of file-only import", async () => {
    mockDetected(["zoom"]);

    renderImports();

    fireEvent.click(await screen.findByRole("button", { name: "Connect" }));

    await waitFor(() => {
      expect(mocks.connectNangoImport).toHaveBeenCalledOnce();
    });
    expect(mocks.connectConnectedImport).not.toHaveBeenCalled();
    expect(
      await screen.findByRole("button", { name: "Sync now" }),
    ).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Connect" })).toBeNull();
    expect(screen.getByText("Connected")).toBeTruthy();
    expect(
      screen.queryByText(/Direct connection is not available yet/i),
    ).toBeNull();
  });

  it("keeps each provider's sync warnings and errors in its own row", async () => {
    mockDetected(["google-meet", "zoom"]);
    mocks.connections = [
      { connection_id: "meet-1", integration_id: "google-meet" },
      { connection_id: "zoom-1", integration_id: "zoom" },
    ];
    mocks.sync.mockImplementation(async (providerId: string) => {
      if (providerId === "zoom") throw new Error("Zoom sync failed");
      return {
        result: {
          discovered: 0,
          imported: 0,
          matched: 0,
          conflicts: 0,
          errors: 0,
        },
        warnings: ["Meet transcripts unavailable"],
      };
    });
    renderImports();
    const meet = within(
      await screen.findByRole("group", { name: "Google Meet" }),
    );
    const zoom = within(screen.getByRole("group", { name: "Zoom" }));
    expect(meet.queryByText("Meet transcripts unavailable")).toBeNull();
    const meetToggle = meet.getByRole("button", { name: "Google Meet" });
    expect(meetToggle.getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(meetToggle);
    fireEvent.click(zoom.getByRole("button", { name: "Zoom" }));
    expect(meetToggle.getAttribute("aria-expanded")).toBe("true");
    expect(await meet.findByText("Meet transcripts unavailable")).toBeTruthy();
    expect(await zoom.findByText("Zoom sync failed")).toBeTruthy();
    expect(meet.queryByText("Zoom sync failed")).toBeNull();
    expect(zoom.queryByText("Meet transcripts unavailable")).toBeNull();
    fireEvent.click(meetToggle);
    expect(meetToggle.getAttribute("aria-expanded")).toBe("false");
    expect(meet.queryByText("Meet transcripts unavailable")).toBeNull();
    expect(screen.queryByText("Everything is already here.")).toBeNull();
  });

  it("shows a completed file import even when all counts are zero", async () => {
    mockDetected(["slack-huddles"]);
    mocks.importMeetingFiles.mockResolvedValue({
      discovered: 0,
      imported: 0,
      matched: 0,
      conflicts: 0,
      errors: 0,
    });
    renderImports();
    fireEvent.click(
      await screen.findByRole("button", { name: "Choose files" }),
    );
    expect(
      screen
        .getByRole("button", { name: "Slack Huddles" })
        .getAttribute("aria-expanded"),
    ).toBe("false");
    fireEvent.click(screen.getByRole("button", { name: "Slack Huddles" }));
    expect(
      await screen.findByText("Last import: 0 added, 0 unchanged"),
    ).toBeTruthy();
  });

  it("does not show an import result when the file picker is cancelled", async () => {
    mockDetected(["slack-huddles"]);
    mocks.selectFiles.mockResolvedValue(null);
    renderImports();
    fireEvent.click(
      await screen.findByRole("button", { name: "Choose files" }),
    );
    await waitFor(() => expect(mocks.selectFiles).toHaveBeenCalledOnce());
    expect(mocks.importMeetingFiles).not.toHaveBeenCalled();
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("replaces file import counts when a newer sync finishes", async () => {
    mockDetected(["zoom"]);
    mocks.connections = [{ connection_id: "zoom-1", integration_id: "zoom" }];
    mocks.importMeetingFiles.mockResolvedValue({
      discovered: 2,
      imported: 2,
      matched: 0,
      conflicts: 0,
      errors: 0,
    });
    renderImports();
    const button = await screen.findByRole("button", { name: "Sync now" });
    await waitFor(() => expect(button.hasAttribute("disabled")).toBe(false));
    fireEvent.pointerDown(screen.getByRole("button", { name: "More options" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Use files" }));
    fireEvent.click(screen.getByRole("button", { name: "Zoom" }));
    expect(
      await screen.findByText("Last import: 2 added, 0 unchanged"),
    ).toBeTruthy();
    mocks.sync.mockResolvedValue({
      result: {
        discovered: 3,
        imported: 1,
        matched: 2,
        conflicts: 0,
        errors: 0,
      },
      warnings: [],
    });
    fireEvent.click(button);
    expect(
      await screen.findByText("Last import: 1 added, 2 unchanged"),
    ).toBeTruthy();
    expect(screen.queryByText("Last import: 2 added, 0 unchanged")).toBeNull();
  });

  it("shows sync progress and blocks repeat clicks until syncing finishes", async () => {
    mockDetected(["zoom"]);
    mocks.connections = [{ connection_id: "zoom-1", integration_id: "zoom" }];
    renderImports();
    const button = await screen.findByRole("button", { name: "Sync now" });
    await waitFor(() => expect(button.hasAttribute("disabled")).toBe(false));
    let finish!: (value: unknown) => void;
    mocks.sync.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    fireEvent.click(button);
    await waitFor(() =>
      expect(
        screen.getByRole("progressbar", { name: "Sync now" }),
      ).toBeTruthy(),
    );
    expect(button.hasAttribute("disabled")).toBe(true);
    expect(button.getAttribute("aria-busy")).toBe("true");
    expect(
      screen
        .getByRole("button", { name: "More options" })
        .hasAttribute("disabled"),
    ).toBe(true);
    fireEvent.click(button);
    expect(mocks.sync).toHaveBeenCalledTimes(2);
    await act(async () =>
      finish({
        result: {
          discovered: 0,
          imported: 0,
          matched: 0,
          conflicts: 0,
          errors: 0,
        },
        warnings: [],
      }),
    );
    await waitFor(() => expect(screen.queryByRole("progressbar")).toBeNull());
    expect(button.hasAttribute("disabled")).toBe(false);
    expect(
      screen
        .getByRole("button", { name: "More options" })
        .hasAttribute("disabled"),
    ).toBe(false);
  });

  it("connects Plaud by running the local CLI instead of file-only import", async () => {
    mockDetected(["plaud"]);
    mocks.connectConnectedImport.mockResolvedValue({
      providerId: "plaud",
      clientId: "ada@example.com",
      tokenJson: "{}",
    });

    renderImports();

    fireEvent.click(await screen.findByRole("button", { name: "Connect" }));

    await waitFor(() => {
      expect(mocks.connectConnectedImport).toHaveBeenCalledOnce();
    });
    expect(mocks.connectNangoImport).not.toHaveBeenCalled();
    expect(screen.getByText("Connected")).toBeTruthy();
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

    fireEvent.click(await screen.findByRole("button", { name: "Connect" }));

    await waitFor(() => {
      expect(mocks.connectConnectedImport).toHaveBeenCalledOnce();
    });
    expect(mocks.connectNangoImport).not.toHaveBeenCalled();
    expect(screen.getByText("Connected")).toBeTruthy();
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
