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
  listCrmProviders: vi.fn(),
  readCrmCredentials: vi.fn(),
  connectCrm: vi.fn(),
  cancelCrmConnection: vi.fn(),
  disconnectCrm: vi.fn(),
  verifyCrmConnection: vi.fn(),
}));

vi.mock("@anlg/plugin-importer", () => ({
  commands: { listCrmProviders: mocks.listCrmProviders },
}));

vi.mock("~/crm/connection", async () => {
  const actual =
    await vi.importActual<typeof import("~/crm/connection")>(
      "~/crm/connection",
    );
  return {
    ...actual,
    readCrmCredentials: mocks.readCrmCredentials,
    crmCredentialsQueryOptions: (providerId: string) => ({
      queryKey: actual.crmCredentialsQueryKey(providerId),
      queryFn: () => mocks.readCrmCredentials(providerId),
      staleTime: Infinity,
    }),
    connectCrm: mocks.connectCrm,
    cancelCrmConnection: mocks.cancelCrmConnection,
    disconnectCrm: mocks.disconnectCrm,
    verifyCrmConnection: mocks.verifyCrmConnection,
  };
});

import { SettingsCrm } from "./index";

const credentials = {
  providerId: "acme",
  clientId: "client",
  clientSecret: null,
  tokenJson: "{}",
  tokenReceivedAt: null,
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
    mocks.readCrmCredentials.mockResolvedValue(null);
  });

  afterEach(() => {
    cleanup();
  });

  it("shows an empty state when no CRM providers exist", async () => {
    mocks.listCrmProviders.mockResolvedValue([]);
    renderPage();
    expect(
      await screen.findByText("No CRM integrations are available yet."),
    ).toBeTruthy();
  });

  it("connects a dynamic-registration provider without client fields", async () => {
    mocks.listCrmProviders.mockResolvedValue([
      { id: "acme", name: "Acme", requiresClient: false, redirectUri: null },
    ]);
    mocks.connectCrm.mockResolvedValue(credentials);
    renderPage();

    await screen.findByText("Not connected");
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));

    await waitFor(() =>
      expect(mocks.connectCrm).toHaveBeenCalledWith(
        expect.objectContaining({ id: "acme" }),
        null,
        expect.any(AbortSignal),
      ),
    );
    expect(await screen.findByText("Connected")).toBeTruthy();
    expect(screen.queryByLabelText("Acme client ID")).toBeNull();
  });

  it("requires a client ID for preregistered providers and passes it through", async () => {
    mocks.listCrmProviders.mockResolvedValue([
      {
        id: "acme",
        name: "Acme",
        requiresClient: true,
        redirectUri: "http://127.0.0.1:4242/callback",
      },
    ]);
    mocks.connectCrm.mockResolvedValue(credentials);
    const { container } = renderPage();

    await screen.findByText("Not connected");
    const connect = screen.getByRole("button", { name: "Connect" });
    expect(connect).toHaveProperty("disabled", true);
    expect(container.textContent).toContain("http://127.0.0.1:4242/callback");

    fireEvent.change(screen.getByLabelText("Acme client ID"), {
      target: { value: "my-client" },
    });
    fireEvent.change(screen.getByLabelText("Acme client secret"), {
      target: { value: "shh" },
    });
    fireEvent.click(connect);

    await waitFor(() =>
      expect(mocks.connectCrm).toHaveBeenCalledWith(
        expect.objectContaining({ id: "acme" }),
        { clientId: "my-client", clientSecret: "shh" },
        expect.any(AbortSignal),
      ),
    );
  });

  it("tests and disconnects an existing connection", async () => {
    mocks.listCrmProviders.mockResolvedValue([
      { id: "acme", name: "Acme", requiresClient: false, redirectUri: null },
    ]);
    mocks.readCrmCredentials.mockResolvedValue(credentials);
    mocks.verifyCrmConnection.mockResolvedValue(credentials);
    mocks.disconnectCrm.mockResolvedValue(undefined);
    renderPage();

    fireEvent.click(await screen.findByRole("button", { name: "Test" }));
    expect(await screen.findByText("Connected and working")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Disconnect" }));
    await waitFor(() =>
      expect(mocks.disconnectCrm).toHaveBeenCalledWith("acme"),
    );
    expect(await screen.findByText("Not connected")).toBeTruthy();
  });

  it("surfaces connection errors", async () => {
    mocks.listCrmProviders.mockResolvedValue([
      { id: "acme", name: "Acme", requiresClient: false, redirectUri: null },
    ]);
    mocks.connectCrm.mockRejectedValue(new Error("Acme connection failed"));
    renderPage();

    await screen.findByText("Not connected");
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    expect((await screen.findByRole("alert")).textContent).toBe(
      "Acme connection failed",
    );
  });
});
