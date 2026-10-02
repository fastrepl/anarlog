import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { HumanRecord } from "~/contacts/queries";

const mocks = vi.hoisted(() => ({
  connections: { data: [] as unknown[], isPending: false },
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({
    session: { user: { id: "self" } },
    getHeaders: () => ({}),
  }),
}));
vi.mock("~/auth/useConnections", () => ({
  useConnections: () => mocks.connections,
}));
vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (select: (state: unknown) => unknown) =>
    select({ openNew: vi.fn() }),
}));
vi.mock("~/crm/connection", () => ({
  crmProvidersQueryOptions: () => ({
    queryKey: ["crm", "providers"],
    queryFn: () => [
      { id: "attio", name: "Attio", nangoIntegrationId: "attio" },
    ],
  }),
  findCrmConnection: (
    provider: { nangoIntegrationId: string },
    connections: { integration_id: string }[] | undefined,
  ) =>
    connections?.find(
      (item) => item.integration_id === provider.nangoIntegrationId,
    ),
  nangoConnectionIsReady: (connection: { status?: string } | undefined) =>
    !!connection && connection.status !== "reconnect_required",
}));

import { EnrichContactButton, useCrmEnrichment } from "./enrich-contact";

function human(): HumanRecord {
  return {
    id: "h1",
    name: "Alice",
    pinned: false,
    userId: "self",
    email: "alice@example.com",
    phone: "",
    jobTitle: "",
    organizationId: "",
    createdAt: "",
    linkedinUsername: "",
    memo: "",
    pinOrder: 0,
    avatarDataUrl: null,
    summary: null,
  };
}

function Toolbar() {
  const contact = human();
  const enrichment = useCrmEnrichment({ human: contact, ownerUserId: "self" });
  return <EnrichContactButton enrichment={enrichment} human={contact} />;
}

function renderButton() {
  const queryClient = new QueryClient();
  return render(
    <QueryClientProvider client={queryClient}>
      <Toolbar />
    </QueryClientProvider>,
  );
}

afterEach(cleanup);
beforeEach(() => {
  mocks.connections = { data: [], isPending: false };
});

describe("EnrichContactButton", () => {
  it("is disabled and explains how to connect when no CRM is connected", async () => {
    renderButton();
    const button = await screen.findByRole("button", {
      name: "Enrich contact from CRM",
    });
    expect((button as HTMLButtonElement).disabled).toBe(true);

    fireEvent.pointerEnter(button.parentElement!);
    expect(await screen.findByText("Connect a CRM")).not.toBeNull();
    expect(screen.getByText(/in Settings › CRM/)).not.toBeNull();

    fireEvent.pointerLeave(button.parentElement!);
  });

  it("is enabled when a matching CRM connection exists", async () => {
    mocks.connections = {
      data: [
        {
          integration_id: "attio",
          connection_id: "conn-1",
          status: "ok",
        },
      ],
      isPending: false,
    };
    renderButton();
    const button = await screen.findByRole("button", {
      name: "Enrich contact from CRM",
    });
    await waitFor(() =>
      expect((button as HTMLButtonElement).disabled).toBe(false),
    );

    fireEvent.pointerEnter(button.parentElement!);
    expect(screen.queryByText("Connect a CRM")).toBeNull();
  });
});
