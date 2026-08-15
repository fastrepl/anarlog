import { afterEach, describe, expect, test, vi } from "vitest";

import { createTaskScheduler } from "./task-scheduler";

describe("TaskScheduler", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  test("has no idle heartbeat", () => {
    vi.useFakeTimers();
    const scheduler = createTaskScheduler().start();

    expect(vi.getTimerCount()).toBe(0);

    scheduler.stop();
  });

  test("uses one timer for the exact next deadline", async () => {
    vi.useFakeTimers();
    const task = vi.fn(async () => undefined);
    const scheduler = createTaskScheduler();
    scheduler.setTask("scheduled", task);
    scheduler.scheduleTaskRun("scheduled", undefined, 60_000);
    scheduler.start();

    expect(vi.getTimerCount()).toBe(1);
    await vi.advanceTimersByTimeAsync(59_999);
    expect(task).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);
    expect(task).toHaveBeenCalledOnce();

    scheduler.stop();
  });

  test("reschedules repeating work from its completion time", async () => {
    vi.useFakeTimers();
    const task = vi.fn(async () => undefined);
    const scheduler = createTaskScheduler();
    scheduler.setTask("repeating", task);
    scheduler.scheduleTaskRun("repeating", undefined, 0, {
      repeatDelay: 30_000,
    });
    scheduler.start();

    await vi.advanceTimersByTimeAsync(0);
    expect(task).toHaveBeenCalledOnce();
    expect(vi.getTimerCount()).toBe(1);
    await vi.advanceTimersByTimeAsync(29_999);
    expect(task).toHaveBeenCalledOnce();
    await vi.advanceTimersByTimeAsync(1);
    expect(task).toHaveBeenCalledTimes(2);

    scheduler.stop();
  });

  test("releases queued runs when their task is removed", () => {
    vi.useFakeTimers();
    const scheduler = createTaskScheduler().start();
    scheduler.setTask("removed", vi.fn());
    scheduler.scheduleTaskRun("removed", undefined, 60_000);

    scheduler.delTask("removed");

    expect(scheduler.getScheduledTaskRunIds()).toEqual([]);
    expect(vi.getTimerCount()).toBe(0);
    scheduler.stop();
  });
});
