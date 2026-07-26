import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { createManager } from "tinytick";
import { Provider as TinyTickProvider } from "tinytick/ui-react";
import { afterEach, describe, expect, test, vi } from "vitest";

import { SyncProvider, useSync } from "./context";

import { CALENDAR_SYNC_TASK_ID } from "~/services/calendar";

function StatusHarness() {
  const { scheduleSync, status } = useSync();

  return (
    <button type="button" onClick={scheduleSync}>
      {status}
    </button>
  );
}

describe("SyncProvider", () => {
  const managers: ReturnType<typeof createManager>[] = [];

  afterEach(() => {
    cleanup();
    for (const manager of managers) {
      manager.stop(true);
    }
    managers.length = 0;
    vi.useRealTimers();
  });

  test("keeps a newly scheduled sync in the scheduled state", () => {
    const manager = createManager();
    managers.push(manager);
    manager.setTask(CALENDAR_SYNC_TASK_ID, async () => undefined);

    render(
      <TinyTickProvider manager={manager}>
        <SyncProvider>
          <StatusHarness />
        </SyncProvider>
      </TinyTickProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "idle" }));

    expect(screen.getByRole("button", { name: "scheduled" })).toBeDefined();
  });

  test("reflects a calendar sync scheduled outside the provider", () => {
    const manager = createManager();
    managers.push(manager);
    manager.setTask(CALENDAR_SYNC_TASK_ID, async () => undefined);
    manager.scheduleTaskRun(CALENDAR_SYNC_TASK_ID);

    render(
      <TinyTickProvider manager={manager}>
        <SyncProvider>
          <StatusHarness />
        </SyncProvider>
      </TinyTickProvider>,
    );

    expect(screen.getByRole("button", { name: "scheduled" })).toBeDefined();
  });

  test("moves from scheduled to syncing to idle", async () => {
    vi.useFakeTimers();
    const manager = createManager();
    managers.push(manager);
    let finishTask = () => {};
    manager.setTask(
      CALENDAR_SYNC_TASK_ID,
      async () =>
        await new Promise<void>((resolve) => {
          finishTask = resolve;
        }),
    );

    render(
      <TinyTickProvider manager={manager}>
        <SyncProvider>
          <StatusHarness />
        </SyncProvider>
      </TinyTickProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "idle" }));
    expect(screen.getByRole("button", { name: "scheduled" })).toBeDefined();

    manager.start();
    await act(async () => await vi.advanceTimersByTimeAsync(100));
    expect(screen.getByRole("button", { name: "syncing" })).toBeDefined();

    await act(async () => {
      finishTask();
      await Promise.resolve();
    });
    expect(screen.getByRole("button", { name: "idle" })).toBeDefined();
  });
});
