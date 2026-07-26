import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  session: {
    access_token: "access-token",
    user: { id: "owner-1", is_anonymous: false },
  },
  createClient: vi.fn(
    (_input: { apiBaseUrl: string; getAccessToken: () => string }) => ({}),
  ),
  stopRunner: vi.fn(),
  startRunner: vi.fn((_dependencies: unknown) => mocks.stopRunner),
}));

vi.mock("./client", () => ({
  createAttachmentBackupClient: mocks.createClient,
}));

vi.mock("./runner", () => ({
  startAttachmentTransferRunner: mocks.startRunner,
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({
    session: mocks.session,
  }),
}));

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => ({ isPaid: true }),
}));

vi.mock("~/env", () => ({
  env: {
    VITE_API_URL: "https://api.example.com",
    VITE_SUPABASE_URL: "https://project.supabase.co",
  },
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: () => true,
}));

import { AttachmentTransferLifecycle } from "./lifecycle";

describe("AttachmentTransferLifecycle", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
    mocks.session = {
      access_token: "access-token",
      user: { id: "owner-1", is_anonymous: false },
    };
  });

  it("refreshes an open editor attachment resolver after a restore", async () => {
    const queryClient = new QueryClient();
    const invalidateQueries = vi.spyOn(queryClient, "invalidateQueries");

    render(
      <QueryClientProvider client={queryClient}>
        <AttachmentTransferLifecycle />
      </QueryClientProvider>,
    );

    await waitFor(() => expect(mocks.startRunner).toHaveBeenCalledOnce());
    const dependencies = mocks.startRunner.mock.calls[0]![0] as {
      onAttachmentRestored: (attachment: { sessionId: string }) => void;
    };
    dependencies.onAttachmentRestored({ sessionId: "session-1" });

    expect(invalidateQueries).toHaveBeenCalledWith({
      queryKey: ["session", "session-1", "attachment-paths"],
    });
  });

  it("keeps transfers running across token refreshes", async () => {
    const queryClient = new QueryClient();
    const view = render(
      <QueryClientProvider client={queryClient}>
        <AttachmentTransferLifecycle />
      </QueryClientProvider>,
    );

    await waitFor(() => expect(mocks.startRunner).toHaveBeenCalledOnce());
    const clientInput = mocks.createClient.mock.calls[0]![0];

    mocks.session = {
      ...mocks.session,
      access_token: "refreshed-token",
    };
    view.rerender(
      <QueryClientProvider client={queryClient}>
        <AttachmentTransferLifecycle />
      </QueryClientProvider>,
    );

    expect(mocks.startRunner).toHaveBeenCalledOnce();
    expect(mocks.stopRunner).not.toHaveBeenCalled();
    expect(clientInput.getAccessToken()).toBe("refreshed-token");
  });
});
