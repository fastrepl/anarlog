import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  create: vi.fn(),
  import: vi.fn(),
  inspect: vi.fn(),
}));

vi.mock("@hypr/plugin-db", () => ({
  createE2eeIdentity: mocks.create,
  importE2eeIdentity: mocks.import,
  inspectE2eeRecoveryKey: mocks.inspect,
}));

import { E2eeSetupDialog } from "./e2ee-setup";

describe("E2eeSetupDialog", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
    vi.unstubAllGlobals();
  });

  const renderDialog = (onReady = vi.fn()) => {
    mocks.inspect.mockResolvedValue({ keyId: "abcdefghijklmnopqrstuv" });
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(
          new Response(JSON.stringify({ keyId: "abcdefghijklmnopqrstuv" }), {
            status: 200,
          }),
        ),
      ),
    );
    const queryClient = new QueryClient({
      defaultOptions: { mutations: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <E2eeSetupDialog
          open
          onOpenChange={vi.fn()}
          accountUserId="11111111-1111-4111-8111-111111111111"
          accessToken="access-token"
          onReady={onReady}
        />
      </QueryClientProvider>,
    );
    return onReady;
  };

  it("requires the generated recovery key to be acknowledged before enabling sync", async () => {
    const onReady = vi.fn();
    mocks.create.mockResolvedValue(
      "anarlog-e2ee-v1:abcdefghijklmnopqrstuvwxyz0123456789ABCDEFG",
    );
    mocks.import.mockResolvedValue(undefined);
    renderDialog(onReady);

    fireEvent.click(screen.getByText("Create a recovery key"));
    expect(await screen.findByText(/anarlog-e2ee-v1:/)).toBeTruthy();
    expect(onReady).not.toHaveBeenCalled();

    fireEvent.click(screen.getByText("I saved it"));
    await waitFor(() => expect(onReady).toHaveBeenCalledTimes(1));
  });

  it("imports an existing recovery key for another device", async () => {
    const onReady = vi.fn();
    mocks.import.mockResolvedValue(undefined);
    renderDialog(onReady);

    fireEvent.click(screen.getByText("Use an existing key"));
    fireEvent.change(screen.getByLabelText("Recovery key"), {
      target: { value: "anarlog-e2ee-v1:existing" },
    });
    fireEvent.click(screen.getByText("Unlock sync"));

    await waitFor(() =>
      expect(mocks.import).toHaveBeenCalledWith(
        "11111111-1111-4111-8111-111111111111",
        "anarlog-e2ee-v1:existing",
      ),
    );
    expect(onReady).toHaveBeenCalledTimes(1);
  });

  it("does not store a recovery key rejected by the account identity", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.resolve(new Response(null, { status: 409 }))),
    );
    mocks.inspect.mockResolvedValue({ keyId: "abcdefghijklmnopqrstuv" });
    const queryClient = new QueryClient({
      defaultOptions: { mutations: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <E2eeSetupDialog
          open
          onOpenChange={vi.fn()}
          accountUserId="11111111-1111-4111-8111-111111111111"
          accessToken="access-token"
          onReady={vi.fn()}
        />
      </QueryClientProvider>,
    );

    fireEvent.click(screen.getByText("Use an existing key"));
    fireEvent.change(screen.getByLabelText("Recovery key"), {
      target: { value: "anarlog-e2ee-v1:wrong" },
    });
    fireEvent.click(screen.getByText("Unlock sync"));

    expect(
      await screen.findByText(/already uses another recovery key/i),
    ).toBeTruthy();
    expect(mocks.import).not.toHaveBeenCalled();
  });
});
