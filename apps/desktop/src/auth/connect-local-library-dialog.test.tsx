import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  getSession: vi.fn(),
  connect: vi.fn(),
  error: vi.fn(),
}));
vi.mock("./client", () => ({
  supabase: { auth: { getSession: mocks.getSession } },
}));
vi.mock("@anlg/plugin-db", () => ({
  execute: mocks.execute,
  connectLocalLibrary: mocks.connect,
}));
vi.mock("@anlg/ui/components/ui/toast", () => ({
  toast: { error: mocks.error },
}));

import { ConnectLocalLibraryDialog } from "./connect-local-library-dialog";

beforeEach(() => {
  vi.clearAllMocks();
  mocks.getSession.mockResolvedValue({
    data: { session: { user: { id: "account-b" } } },
    error: null,
  });
  mocks.execute.mockResolvedValue([{ workspace_id: "local-library" }]);
  mocks.connect.mockResolvedValue(undefined);
});
afterEach(cleanup);

function renderDialog() {
  const onConnected = vi.fn().mockResolvedValue(undefined);
  const onOpenChange = vi.fn();
  render(
    <QueryClientProvider
      client={
        new QueryClient({
          defaultOptions: {
            queries: { retry: false },
            mutations: { retry: false },
          },
        })
      }
    >
      <ConnectLocalLibraryDialog
        open
        accountUserId="account-b"
        email="b@example.com"
        onOpenChange={onOpenChange}
        onConnected={onConnected}
      />
    </QueryClientProvider>,
  );
  return { onConnected, onOpenChange };
}

it("waits for consent and resumes sync only after the selected library is connected", async () => {
  const { onConnected, onOpenChange } = renderDialog();
  await waitFor(() =>
    expect(
      screen
        .getByRole("button", { name: "Connect library" })
        .hasAttribute("disabled"),
    ).toBe(false),
  );
  expect(mocks.connect).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Connect library" }));
  await waitFor(() => expect(onConnected).toHaveBeenCalledOnce());
  expect(mocks.connect).toHaveBeenCalledWith("account-b", "local-library");
  expect(onOpenChange).toHaveBeenCalledWith(false);
});

it("keeps the library local when confirmation is cancelled", async () => {
  const { onConnected, onOpenChange } = renderDialog();
  fireEvent.click(screen.getByRole("button", { name: "Keep using locally" }));
  expect(onOpenChange).toHaveBeenCalledWith(false);
  expect(mocks.connect).not.toHaveBeenCalled();
  expect(onConnected).not.toHaveBeenCalled();
});

it("does not start sync when the native connection fails", async () => {
  mocks.connect.mockRejectedValueOnce(new Error("stale library"));
  const { onConnected, onOpenChange } = renderDialog();
  await waitFor(() =>
    expect(
      screen
        .getByRole("button", { name: "Connect library" })
        .hasAttribute("disabled"),
    ).toBe(false),
  );
  fireEvent.click(screen.getByRole("button", { name: "Connect library" }));
  await waitFor(() => expect(mocks.error).toHaveBeenCalledOnce());
  expect(onConnected).not.toHaveBeenCalled();
  expect(onOpenChange).not.toHaveBeenCalled();
});

it("rejects confirmation after the signed-in account changes", async () => {
  mocks.getSession.mockResolvedValueOnce({
    data: { session: { user: { id: "account-c" } } },
    error: null,
  });
  const { onConnected } = renderDialog();
  await waitFor(() =>
    expect(
      screen
        .getByRole("button", { name: "Connect library" })
        .hasAttribute("disabled"),
    ).toBe(false),
  );
  fireEvent.click(screen.getByRole("button", { name: "Connect library" }));
  await waitFor(() => expect(mocks.error).toHaveBeenCalledOnce());
  expect(mocks.connect).not.toHaveBeenCalled();
  expect(onConnected).not.toHaveBeenCalled();
});

it("closes after native success and reports a subsequent sync refresh failure accurately", async () => {
  const { onConnected, onOpenChange } = renderDialog();
  onConnected.mockRejectedValueOnce(new Error("refresh failed"));
  await waitFor(() =>
    expect(
      screen
        .getByRole("button", { name: "Connect library" })
        .hasAttribute("disabled"),
    ).toBe(false),
  );
  fireEvent.click(screen.getByRole("button", { name: "Connect library" }));
  await waitFor(() =>
    expect(mocks.error).toHaveBeenCalledWith(
      "Library connected, but sync could not restart. Try again in sync settings.",
    ),
  );
  expect(onOpenChange).toHaveBeenCalledWith(false);
  expect(mocks.connect).toHaveBeenCalledOnce();
});
