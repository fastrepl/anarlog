import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { afterEach, describe, expect, test, vi } from "vitest";

import { SyncProvider, useSync } from "./context";

import { CALENDAR_SYNC_TASK_ID } from "~/services/calendar";
import {
  createTaskScheduler,
  TaskSchedulerProvider,
} from "~/services/task-scheduler";

function StatusHarness() {
  const { canSync, scheduleSync, status } = useSync();

  return (
    <button type="button" disabled={!canSync} onClick={scheduleSync}>
      {status}
    </button>
  );
}

describe("SyncProvider", () => {
  const managers: ReturnType<typeof createTaskScheduler>[] = [];

  afterEach(() => {
    cleanup();
    for (const manager of managers) {
      manager.stop();
    }
    managers.length = 0;
    vi.useRealTimers();
  });

  test("disables sync without a scheduler", () => {
    render(
      <SyncProvider>
        <StatusHarness />
      </SyncProvider>,
    );

    expect(screen.getByRole("button", { name: "idle" })).toHaveProperty(
      "disabled",
      true,
    );
  });

  test("keeps a newly scheduled sync in the scheduled state", () => {
    const manager = createTaskScheduler();
    managers.push(manager);
    manager.setTask(CALENDAR_SYNC_TASK_ID, async () => undefined);

    render(
      <TaskSchedulerProvider scheduler={manager}>
        <SyncProvider>
          <StatusHarness />
        </SyncProvider>
      </TaskSchedulerProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "idle" }));

    expect(screen.getByRole("button", { name: "scheduled" })).toBeDefined();
  });

  test("reflects a calendar sync scheduled outside the provider", () => {
    const manager = createTaskScheduler();
    managers.push(manager);
    manager.setTask(CALENDAR_SYNC_TASK_ID, async () => undefined);
    manager.scheduleTaskRun(CALENDAR_SYNC_TASK_ID);

    render(
      <TaskSchedulerProvider scheduler={manager}>
        <SyncProvider>
          <StatusHarness />
        </SyncProvider>
      </TaskSchedulerProvider>,
    );

    expect(screen.getByRole("button", { name: "scheduled" })).toBeDefined();
  });

  test("moves from scheduled to syncing to idle", async () => {
    vi.useFakeTimers();
    const manager = createTaskScheduler();
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
      <TaskSchedulerProvider scheduler={manager}>
        <SyncProvider>
          <StatusHarness />
        </SyncProvider>
      </TaskSchedulerProvider>,
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
