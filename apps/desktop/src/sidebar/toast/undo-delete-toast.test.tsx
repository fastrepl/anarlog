import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  message: vi.fn(),
  error: vi.fn(),
  dismiss: vi.fn(),
  openCurrent: vi.fn(),
  restoreDeletedSession: vi.fn(),
}));

vi.mock("@hypr/ui/components/ui/toast", () => ({
  sonnerToast: {
    message: mocks.message,
    error: mocks.error,
    dismiss: mocks.dismiss,
  },
}));

vi.mock("~/session/queries", () => ({
  restoreDeletedSession: mocks.restoreDeletedSession,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: { openCurrent: () => void }) => unknown) =>
    selector({ openCurrent: mocks.openCurrent }),
}));

import { UndoDeleteToast } from "./undo-delete-toast";

import { useUndoDelete } from "~/store/zustand/undo-delete";

describe("UndoDeleteToast", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    useUndoDelete.setState({ pendingDeletions: {} });
    mocks.message.mockClear();
    mocks.error.mockClear();
    mocks.dismiss.mockClear();
    mocks.openCurrent.mockClear();
    mocks.restoreDeletedSession.mockClear();
  });

  afterEach(() => {
    for (const pending of Object.values(
      useUndoDelete.getState().pendingDeletions,
    )) {
      if (pending.timeoutId) clearTimeout(pending.timeoutId);
    }
    useUndoDelete.setState({ pendingDeletions: {} });
    cleanup();
    vi.useRealTimers();
  });

  it("renders undo deletion through Sonner", () => {
    act(() => {
      useUndoDelete.getState().addDeletion({
        session: { id: "session-1", title: "Design sync" },
        tombstone: "tombstone",
        deletedAt: Date.now(),
      });
    });

    const queryClient = new QueryClient();
    const view = render(
      <QueryClientProvider client={queryClient}>
        <UndoDeleteToast />
      </QueryClientProvider>,
    );

    expect(mocks.message).toHaveBeenCalledWith(
      "Deleting Design sync",
      expect.objectContaining({
        id: "undo-delete:session-1",
        duration: Infinity,
        actionButtonStyle: {
          background: "hsl(var(--destructive))",
          color: "hsl(var(--destructive-foreground))",
        },
        action: expect.objectContaining({ label: "Delete" }),
        cancel: expect.objectContaining({ label: "Undo" }),
      }),
    );

    view.unmount();
    expect(mocks.dismiss).toHaveBeenCalledWith("undo-delete:session-1");
  });

  it("updates a batch toast when later deletions join the batch", async () => {
    const queryClient = new QueryClient();
    render(
      <QueryClientProvider client={queryClient}>
        <UndoDeleteToast />
      </QueryClientProvider>,
    );

    act(() => {
      useUndoDelete.getState().addDeletion(
        {
          session: { id: "session-1", title: "Design sync" },
          tombstone: "tombstone-1",
          deletedAt: Date.now(),
        },
        undefined,
        "batch-1",
      );
    });

    expect(mocks.message).toHaveBeenLastCalledWith(
      "Deleting 1 notes",
      expect.objectContaining({ id: "undo-delete:batch-1" }),
    );

    act(() => {
      useUndoDelete.getState().addDeletion(
        {
          session: { id: "session-2", title: "Weekly review" },
          tombstone: "tombstone-2",
          deletedAt: Date.now(),
        },
        undefined,
        "batch-1",
      );
    });

    expect(mocks.message).toHaveBeenLastCalledWith(
      "Deleting 2 notes",
      expect.objectContaining({ id: "undo-delete:batch-1" }),
    );

    const options =
      mocks.message.mock.calls[mocks.message.mock.calls.length - 1][1];
    await act(async () => {
      options.cancel.onClick();
      await Promise.resolve();
    });

    expect(mocks.restoreDeletedSession).toHaveBeenCalledTimes(2);
    expect(mocks.restoreDeletedSession).toHaveBeenNthCalledWith(
      1,
      expect.objectContaining({
        session: expect.objectContaining({ id: "session-1" }),
      }),
    );
    expect(mocks.restoreDeletedSession).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({
        session: expect.objectContaining({ id: "session-2" }),
      }),
    );
  });
});
