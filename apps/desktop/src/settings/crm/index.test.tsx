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
  signedIn: true,
  connections: [] as Array<{
    integration_id: string;
    connection_id: string;
    status: string | null;
  }>,
  providers: [] as Array<{
    id: string;
    name: string;
    nangoIntegrationId: string;
  }>,
  connectCrm: vi.fn(),
  disconnectCrm: vi.fn(),
  verifyCrmConnection: vi.fn(),
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({
    session: mocks.signedIn ? { user: { id: "user-1" } } : null,
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

vi.mock("~/crm/connection", async () => {
  const actual =
    await vi.importActual<typeof import("~/crm/connection")>(
      "~/crm/connection",
    );
  return {
    ...actual,
    crmProvidersQueryOptions: () => ({
      queryKey: ["crm", "providers"],
      queryFn: async () => mocks.providers,
      staleTime: Infinity,
    }),
    connectCrm: mocks.connectCrm,
    disconnectCrm: mocks.disconnectCrm,
    verifyCrmConnection: mocks.verifyCrmConnection,
  };
});

import { SettingsCrm } from "./index";

const provider = {
  id: "acme",
  name: "Acme",
  nangoIntegrationId: "acme-nango",
};

const connection = {
  integration_id: "acme-nango",
  connection_id: "conn-1",
  status: "connected",
};

function renderPage() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  return render(<SettingsCrm />, { wrapper });
}

describe("SettingsCrm", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.signedIn = true;
    mocks.connections = [];
    mocks.providers = [provider];
  });

  afterEach(() => {
    cleanup();
  });

  it("asks the user to sign in when signed out", async () => {
    mocks.signedIn = false;
    renderPage();
    expect(await screen.findByText("Sign in to connect a CRM.")).toBeTruthy();
  });

  it("shows an empty state when no CRM providers exist", async () => {
    mocks.providers = [];
    renderPage();
    expect(
      await screen.findByText("No CRM integrations are available yet."),
    ).toBeTruthy();
  });

  it("starts the Nango connect flow", async () => {
    mocks.connectCrm.mockResolvedValue(connection);
    renderPage();

    await screen.findByText("Not connected");
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));

    await waitFor(() =>
      expect(mocks.connectCrm).toHaveBeenCalledWith(
        expect.objectContaining({ id: "acme" }),
        { Authorization: "Bearer test" },
        expect.any(AbortSignal),
        undefined,
      ),
    );
  });

  it("surfaces connection errors", async () => {
    mocks.connectCrm.mockRejectedValue(new Error("Acme connection failed"));
    renderPage();

    await screen.findByText("Not connected");
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    expect((await screen.findByRole("alert")).textContent).toBe(
      "Acme connection failed",
    );
  });

  it("tests and disconnects an existing connection", async () => {
    mocks.connections = [connection];
    mocks.verifyCrmConnection.mockResolvedValue(undefined);
    mocks.disconnectCrm.mockResolvedValue(undefined);
    renderPage();

    fireEvent.click(await screen.findByRole("button", { name: "Test" }));
    await waitFor(() =>
      expect(mocks.verifyCrmConnection).toHaveBeenCalledWith(
        expect.objectContaining({ id: "acme" }),
        "conn-1",
        { Authorization: "Bearer test" },
      ),
    );
    expect(await screen.findByText("Connected and working")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Disconnect" }));
    await waitFor(() =>
      expect(mocks.disconnectCrm).toHaveBeenCalledWith(
        expect.objectContaining({ nangoIntegrationId: "acme-nango" }),
        "conn-1",
      ),
    );
  });

  it("shows reconnect state for expired connections", async () => {
    mocks.connections = [{ ...connection, status: "reconnect_required" }];
    renderPage();

    expect(await screen.findByText("Reconnect required")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Reconnect" })).toBeTruthy();
  });

  it("passes the existing connection id when reconnecting", async () => {
    mocks.connections = [{ ...connection, status: "reconnect_required" }];
    mocks.connectCrm.mockResolvedValue(connection);
    renderPage();

    await screen.findByText("Reconnect required");
    fireEvent.click(screen.getByRole("button", { name: "Reconnect" }));

    await waitFor(() =>
      expect(mocks.connectCrm).toHaveBeenCalledWith(
        expect.objectContaining({ id: "acme" }),
        { Authorization: "Bearer test" },
        expect.any(AbortSignal),
        "conn-1",
      ),
    );
  });
});
