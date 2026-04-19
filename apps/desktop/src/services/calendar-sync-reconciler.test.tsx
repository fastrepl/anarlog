import { render } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { CalendarSyncReconciler } from "./calendar-sync-reconciler";

const { useStoreMock, reconcileCalendarSessionsMock } = vi.hoisted(() => ({
  useStoreMock: vi.fn(),
  reconcileCalendarSessionsMock: vi.fn(),
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main-store",
  UI: {
    useStore: useStoreMock,
  },
}));

vi.mock("./calendar/reconcile", () => ({
  reconcileCalendarSessions: reconcileCalendarSessionsMock,
}));

type TableName = "events" | "calendars";
type Listener = () => void;

function createStore() {
  const listeners = new Map<TableName, Map<string, Listener>>([
    ["events", new Map()],
    ["calendars", new Map()],
  ]);
  let nextListenerId = 1;

  return {
    addTableListener: vi.fn((tableName: TableName, listener: Listener) => {
      const id = String(nextListenerId++);
      listeners.get(tableName)?.set(id, listener);
      return id;
    }),
    delListener: vi.fn((listenerId: string) => {
      for (const tableListeners of listeners.values()) {
        tableListeners.delete(listenerId);
      }
    }),
    emit(tableName: TableName) {
      for (const listener of listeners.get(tableName)?.values() ?? []) {
        listener();
      }
    },
  };
}

describe("CalendarSyncReconciler", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    useStoreMock.mockReset();
    reconcileCalendarSessionsMock.mockReset();
  });

  test("reconciles once on mount after the debounce window", () => {
    const store = createStore();
    useStoreMock.mockReturnValue(store);

    render(<CalendarSyncReconciler />);

    expect(reconcileCalendarSessionsMock).not.toHaveBeenCalled();

    vi.advanceTimersByTime(50);

    expect(reconcileCalendarSessionsMock).toHaveBeenCalledTimes(1);
    expect(reconcileCalendarSessionsMock).toHaveBeenCalledWith(store);
  });

  test("reconciles after TinyBase applies events table changes", () => {
    const store = createStore();
    useStoreMock.mockReturnValue(store);

    render(<CalendarSyncReconciler />);
    vi.advanceTimersByTime(50);
    reconcileCalendarSessionsMock.mockClear();

    store.emit("events");
    vi.advanceTimersByTime(49);
    expect(reconcileCalendarSessionsMock).not.toHaveBeenCalled();

    vi.advanceTimersByTime(1);
    expect(reconcileCalendarSessionsMock).toHaveBeenCalledTimes(1);
    expect(reconcileCalendarSessionsMock).toHaveBeenCalledWith(store);
  });

  test("debounces near-simultaneous events and calendars reloads", () => {
    const store = createStore();
    useStoreMock.mockReturnValue(store);

    render(<CalendarSyncReconciler />);
    vi.advanceTimersByTime(50);
    reconcileCalendarSessionsMock.mockClear();

    store.emit("events");
    vi.advanceTimersByTime(25);
    store.emit("calendars");
    vi.advanceTimersByTime(49);

    expect(reconcileCalendarSessionsMock).not.toHaveBeenCalled();

    vi.advanceTimersByTime(1);
    expect(reconcileCalendarSessionsMock).toHaveBeenCalledTimes(1);
  });
});
