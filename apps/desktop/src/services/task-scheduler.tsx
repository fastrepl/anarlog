import {
  createContext,
  useCallback,
  useContext,
  useRef,
  useSyncExternalStore,
} from "react";

import { useMountEffect } from "~/shared/hooks/useMountEffect";

const MAX_TIMEOUT_MS = 2_147_483_647;

export type TaskRunConfig = {
  maxDuration?: number;
  maxRetries?: number;
  repeatDelay?: number | null;
  retryDelay?: number | string;
};

export type TaskRunInfo = {
  manager: TaskScheduler;
  taskId: string;
  taskRunId: string;
  arg: unknown;
  retry: number;
  running: boolean;
  nextTimestamp: number;
};

type Task = (
  arg: unknown,
  signal: AbortSignal,
  info: TaskRunInfo,
) => Promise<void> | void;

type TaskDefinition = {
  task: Task;
  config: TaskRunConfig;
};

type TaskRun = {
  id: string;
  taskId: string;
  arg: unknown;
  config: TaskRunConfig;
  nextTimestamp: number;
  retry: number;
  running: boolean;
  retriesRemaining?: number;
  retryDelays?: number[];
  repeatDelay?: number | null;
  timeout?: number;
  controller?: AbortController;
};

type TaskRunListener = (
  scheduler: TaskScheduler,
  taskId: string,
  taskRunId: string,
  running: boolean | undefined,
) => void;

export class TaskScheduler {
  private readonly tasks = new Map<string, TaskDefinition>();
  private readonly runs = new Map<string, TaskRun>();
  private readonly stateListeners = new Set<() => void>();
  private readonly taskRunListeners = new Map<
    number,
    { taskId: string; taskRunId: string | null; listener: TaskRunListener }
  >();
  private nextListenerId = 0;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private timerDeadline: number | null = null;
  private started = false;
  private scheduledSnapshot: readonly string[] = [];
  private runningSnapshot: readonly string[] = [];

  start(): this {
    this.started = true;
    this.scheduleNextWake();
    return this;
  }

  stop(): this {
    this.started = false;
    this.clearTimer();
    return this;
  }

  setTask(
    taskId: string,
    task: Task,
    config: TaskRunConfig = EMPTY_TASK_RUN_CONFIG,
  ): void {
    this.tasks.set(taskId, { task, config });
    this.scheduleNextWake();
  }

  delTask(taskId: string): void {
    this.tasks.delete(taskId);
    for (const run of this.runs.values()) {
      if (run.taskId !== taskId) continue;
      run.controller?.abort();
      this.runs.delete(run.id);
      this.publishTaskRun(run, undefined);
    }
    this.publishState();
    this.scheduleNextWake();
  }

  scheduleTaskRun(
    taskId: string,
    arg?: unknown,
    startAfter = 0,
    config: TaskRunConfig = EMPTY_TASK_RUN_CONFIG,
  ): string {
    const taskRunId = crypto.randomUUID();
    this.runs.set(taskRunId, {
      id: taskRunId,
      taskId,
      arg,
      config,
      nextTimestamp: Date.now() + Math.max(0, startAfter),
      retry: 0,
      running: false,
    });
    this.publishState();
    this.scheduleNextWake();
    return taskRunId;
  }

  delTaskRun(taskRunId: string): void {
    const run = this.runs.get(taskRunId);
    if (!run) return;
    run.controller?.abort();
    this.runs.delete(taskRunId);
    this.publishTaskRun(run, undefined);
    this.publishState();
    this.scheduleNextWake();
  }

  getTaskRunInfo(taskRunId: string): TaskRunInfo | undefined {
    const run = this.runs.get(taskRunId);
    return run ? this.toTaskRunInfo(run) : undefined;
  }

  getScheduledTaskRunIds(): readonly string[] {
    return this.scheduledSnapshot;
  }

  getRunningTaskRunIds(): readonly string[] {
    return this.runningSnapshot;
  }

  subscribe(listener: () => void): () => void {
    this.stateListeners.add(listener);
    return () => this.stateListeners.delete(listener);
  }

  addTaskRunRunningListener(
    taskId: string,
    taskRunId: string | null,
    listener: TaskRunListener,
  ): number {
    const id = this.nextListenerId++;
    this.taskRunListeners.set(id, { taskId, taskRunId, listener });
    return id;
  }

  delListener(listenerId: number): void {
    this.taskRunListeners.delete(listenerId);
  }

  untilTaskRunDone(taskRunId: string): Promise<void> {
    const run = this.runs.get(taskRunId);
    if (!run) return Promise.resolve();
    return new Promise((resolve) => {
      const listenerId = this.addTaskRunRunningListener(
        run.taskId,
        taskRunId,
        (_scheduler, _taskId, _runId, running) => {
          if (running !== undefined) return;
          this.delListener(listenerId);
          resolve();
        },
      );
    });
  }

  private scheduleNextWake(): void {
    if (!this.started) return;
    const nextDeadline = this.getNextDeadline();
    if (nextDeadline === null) {
      this.clearTimer();
      return;
    }
    if (this.timer && this.timerDeadline === nextDeadline) return;

    this.clearTimer();
    this.timerDeadline = nextDeadline;
    this.timer = setTimeout(
      () => {
        this.timer = null;
        this.timerDeadline = null;
        this.tick();
      },
      Math.min(MAX_TIMEOUT_MS, Math.max(0, nextDeadline - Date.now())),
    );
  }

  private clearTimer(): void {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    this.timerDeadline = null;
  }

  private getNextDeadline(): number | null {
    let deadline = Number.POSITIVE_INFINITY;
    for (const run of this.runs.values()) {
      deadline = Math.min(deadline, run.nextTimestamp);
    }
    return Number.isFinite(deadline) ? deadline : null;
  }

  private tick(): void {
    const now = Date.now();
    const due = [...this.runs.values()].filter(
      (run) => run.nextTimestamp <= now,
    );
    for (const run of due) {
      if (run.running) {
        this.timeoutRun(run);
      } else {
        this.startRun(run);
      }
    }
    this.scheduleNextWake();
  }

  private startRun(run: TaskRun): void {
    const definition = this.tasks.get(run.taskId);
    if (!definition) {
      this.runs.delete(run.id);
      this.publishTaskRun(run, undefined);
      this.publishState();
      return;
    }

    const config = { ...definition.config, ...run.config };
    run.retriesRemaining ??= Math.max(0, config.maxRetries ?? 0);
    run.retryDelays ??= parseRetryDelays(config.retryDelay ?? 1_000);
    run.repeatDelay ??= config.repeatDelay ?? null;
    run.timeout = Math.max(1, config.maxDuration ?? 1_000);
    run.running = true;
    const controller = new AbortController();
    run.controller = controller;
    run.nextTimestamp = Date.now() + run.timeout;
    this.publishTaskRun(run, true);
    this.publishState();
    this.scheduleNextWake();

    void Promise.resolve()
      .then(() =>
        definition.task(run.arg, controller.signal, this.toTaskRunInfo(run)),
      )
      .then(
        () => this.finishRun(run.id),
        () => this.failRun(run.id),
      );
  }

  private finishRun(taskRunId: string): void {
    const run = this.runs.get(taskRunId);
    if (!run || !run.running || run.controller?.signal.aborted) return;
    const repeatDelay = run.repeatDelay;
    if (repeatDelay !== null && repeatDelay !== undefined) {
      run.running = false;
      run.controller = undefined;
      run.nextTimestamp = Date.now() + Math.max(0, repeatDelay);
      run.retry = 0;
      run.retriesRemaining = undefined;
      run.retryDelays = undefined;
      run.repeatDelay = undefined;
      run.timeout = undefined;
      this.publishTaskRun(run, false);
    } else {
      this.runs.delete(taskRunId);
      this.publishTaskRun(run, undefined);
    }
    this.publishState();
    this.scheduleNextWake();
  }

  private failRun(taskRunId: string): void {
    const run = this.runs.get(taskRunId);
    if (!run || !run.running || run.controller?.signal.aborted) return;
    this.retryOrDelete(run);
  }

  private timeoutRun(run: TaskRun): void {
    run.controller?.abort();
    this.retryOrDelete(run);
  }

  private retryOrDelete(run: TaskRun): void {
    const retriesRemaining = run.retriesRemaining ?? 0;
    if (retriesRemaining > 0) {
      const retryDelays = run.retryDelays ?? [1_000];
      const retryDelay =
        retryDelays[Math.min(run.retry, retryDelays.length - 1)]!;
      run.retriesRemaining = retriesRemaining - 1;
      run.retry += 1;
      run.running = false;
      run.controller = undefined;
      run.nextTimestamp = Date.now() + retryDelay;
      this.publishTaskRun(run, false);
    } else {
      this.runs.delete(run.id);
      this.publishTaskRun(run, undefined);
    }
    this.publishState();
    this.scheduleNextWake();
  }

  private publishState(): void {
    this.scheduledSnapshot = [...this.runs.values()]
      .filter((run) => !run.running)
      .sort((left, right) => left.nextTimestamp - right.nextTimestamp)
      .map((run) => run.id);
    this.runningSnapshot = [...this.runs.values()]
      .filter((run) => run.running)
      .sort((left, right) => left.nextTimestamp - right.nextTimestamp)
      .map((run) => run.id);
    this.stateListeners.forEach((listener) => listener());
  }

  private publishTaskRun(run: TaskRun, running: boolean | undefined): void {
    for (const entry of this.taskRunListeners.values()) {
      if (
        entry.taskId === run.taskId &&
        (entry.taskRunId === null || entry.taskRunId === run.id)
      ) {
        entry.listener(this, run.taskId, run.id, running);
      }
    }
  }

  private toTaskRunInfo(run: TaskRun): TaskRunInfo {
    return {
      manager: this,
      taskId: run.taskId,
      taskRunId: run.id,
      arg: run.arg,
      retry: run.retry,
      running: run.running,
      nextTimestamp: run.nextTimestamp,
    };
  }
}

const TaskSchedulerContext = createContext<TaskScheduler | null>(null);

export function TaskSchedulerProvider({
  scheduler,
  children,
}: {
  scheduler: TaskScheduler;
  children: React.ReactNode;
}) {
  return (
    <TaskSchedulerContext.Provider value={scheduler}>
      {children}
    </TaskSchedulerContext.Provider>
  );
}

export function useTaskScheduler(): TaskScheduler | null {
  return useContext(TaskSchedulerContext);
}

export function useScheduledTaskRunIds(): readonly string[] {
  const scheduler = useTaskScheduler();
  const subscribe = useCallback(
    (listener: () => void) =>
      scheduler?.subscribe(listener) ?? emptySubscribe(),
    [scheduler],
  );
  const getSnapshot = useCallback(
    () => scheduler?.getScheduledTaskRunIds() ?? EMPTY_TASK_RUN_IDS,
    [scheduler],
  );
  return useSyncExternalStore(subscribe, getSnapshot, emptySnapshot);
}

export function useRunningTaskRunIds(): readonly string[] {
  const scheduler = useTaskScheduler();
  const subscribe = useCallback(
    (listener: () => void) =>
      scheduler?.subscribe(listener) ?? emptySubscribe(),
    [scheduler],
  );
  const getSnapshot = useCallback(
    () => scheduler?.getRunningTaskRunIds() ?? EMPTY_TASK_RUN_IDS,
    [scheduler],
  );
  return useSyncExternalStore(subscribe, getSnapshot, emptySnapshot);
}

export function useRegisterTask(
  taskId: string,
  task: Task,
  config: TaskRunConfig = EMPTY_TASK_RUN_CONFIG,
): void {
  const scheduler = useTaskScheduler();
  const taskRef = useRef(task);
  taskRef.current = task;
  const configRef = useRef(config);
  configRef.current = config;

  useMountEffect(() => {
    if (!scheduler) return;
    scheduler.setTask(
      taskId,
      (arg, signal, info) => taskRef.current(arg, signal, info),
      configRef.current,
    );
    return () => scheduler.delTask(taskId);
  });
}

export function useScheduleTaskRun(
  taskId: string,
  arg?: unknown,
  startAfter = 0,
  config: TaskRunConfig = EMPTY_TASK_RUN_CONFIG,
): void {
  const scheduler = useTaskScheduler();
  const configRef = useRef(config);
  configRef.current = config;

  useMountEffect(() => {
    if (!scheduler) return;
    const taskRunId = scheduler.scheduleTaskRun(
      taskId,
      arg,
      startAfter,
      configRef.current,
    );
    return () => scheduler.delTaskRun(taskRunId);
  });
}

export function useScheduleTaskRunCallback(
  taskId: string,
  arg?: unknown,
  startAfter = 0,
  config: TaskRunConfig = EMPTY_TASK_RUN_CONFIG,
): () => string | undefined {
  const scheduler = useTaskScheduler();
  return useCallback(
    () => scheduler?.scheduleTaskRun(taskId, arg, startAfter, config),
    [arg, config, scheduler, startAfter, taskId],
  );
}

function parseRetryDelays(value: number | string): number[] {
  const values = typeof value === "string" ? value.split(",") : [value];
  const delays = values
    .map(Number)
    .filter((delay) => Number.isFinite(delay) && delay >= 0);
  return delays.length > 0 ? delays : [1_000];
}

const EMPTY_TASK_RUN_IDS: readonly string[] = [];
const EMPTY_TASK_RUN_CONFIG: TaskRunConfig = {};
const emptySnapshot = () => EMPTY_TASK_RUN_IDS;
const emptySubscribe = () => () => {};

export function createTaskScheduler(): TaskScheduler {
  return new TaskScheduler();
}
