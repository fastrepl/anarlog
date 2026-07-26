import { expect } from "vitest";

import type { Tab, useTabs } from ".";

type TabsState = ReturnType<typeof useTabs.getState>;

interface CustomMatchers<R = unknown> {
  toHaveNavigationState: (expected: {
    canGoBack: boolean;
    canGoNext: boolean;
  }) => R;
  toHaveCurrentTab: (expected: Partial<Tab>) => R;
  toHaveHistoryLength: (length: number) => R;
  toHaveLastHistoryEntry: (expected: Partial<Tab>) => R;
  toMatchTabsInOrder: (expected: Array<Partial<Tab>>) => R;
}

declare module "vitest" {
  interface Matchers<T = any> extends CustomMatchers<T> {}
}

expect.extend({
  toHaveNavigationState(
    state: TabsState,
    expected: { canGoBack: boolean; canGoNext: boolean },
  ) {
    const pass =
      state.canGoBack === expected.canGoBack &&
      state.canGoNext === expected.canGoNext;

    return {
      pass,
      message: () =>
        pass
          ? `Expected navigation state not to be { canGoBack: ${expected.canGoBack}, canGoNext: ${expected.canGoNext} }`
          : `Expected navigation state to be { canGoBack: ${expected.canGoBack}, canGoNext: ${expected.canGoNext} }, but got { canGoBack: ${state.canGoBack}, canGoNext: ${state.canGoNext} }`,
      actual: { canGoBack: state.canGoBack, canGoNext: state.canGoNext },
      expected,
    };
  },

  toHaveCurrentTab(state: TabsState, expected: Partial<Tab>) {
    if (!state.currentTab) {
      return {
        pass: false,
        message: () =>
          `Expected currentTab to match ${JSON.stringify(expected)}, but got null`,
        actual: null,
        expected,
      };
    }

    const pass = Object.entries(expected).every(
      ([key, value]) =>
        JSON.stringify(state.currentTab![key as keyof Tab]) ===
        JSON.stringify(value),
    );

    return {
      pass,
      message: () =>
        pass
          ? `Expected currentTab not to match ${JSON.stringify(expected)}`
          : `Expected currentTab to match ${JSON.stringify(expected)}, but got ${JSON.stringify(state.currentTab)}`,
      actual: state.currentTab,
      expected,
    };
  },

  toHaveHistoryLength(state: TabsState, length: number) {
    if (!state.currentTab) {
      return {
        pass: length === 0,
        message: () =>
          `Expected history length to be ${length}, but no current tab`,
        actual: 0,
        expected: length,
      };
    }

    const slotId = state.currentTab.slotId;
    const stack = state.history.get(slotId)?.stack;
    const actualLength = stack?.length ?? 0;
    const pass = actualLength === length;

    return {
      pass,
      message: () =>
        pass
          ? `Expected history length not to be ${length}`
          : `Expected history length to be ${length}, but got ${actualLength}`,
      actual: actualLength,
      expected: length,
    };
  },

  toHaveLastHistoryEntry(state: TabsState, expected: Partial<Tab>) {
    if (!state.currentTab) {
      return {
        pass: false,
        message: () =>
          `Expected history to have last entry matching ${JSON.stringify(expected)}, but no current tab`,
        actual: null,
        expected,
      };
    }

    const slotId = state.currentTab.slotId;
    const stack = state.history.get(slotId)?.stack;

    if (!stack || stack.length === 0) {
      return {
        pass: false,
        message: () =>
          `Expected history to have last entry matching ${JSON.stringify(expected)}, but history is empty`,
        actual: null,
        expected,
      };
    }

    const lastEntry = stack[stack.length - 1];
    const pass = Object.entries(expected).every(
      ([key, value]) =>
        JSON.stringify(lastEntry[key as keyof Tab]) === JSON.stringify(value),
    );

    return {
      pass,
      message: () =>
        pass
          ? `Expected last history entry not to match ${JSON.stringify(expected)}`
          : `Expected last history entry to match ${JSON.stringify(expected)}, but got ${JSON.stringify(lastEntry)}`,
      actual: lastEntry,
      expected,
    };
  },

  toMatchTabsInOrder(state: TabsState, expected: Array<Partial<Tab>>) {
    const { tabs } = state;

    if (tabs.length !== expected.length) {
      return {
        pass: false,
        message: () =>
          `Expected ${expected.length} tabs, but got ${tabs.length}`,
        actual: tabs.length,
        expected: expected.length,
      };
    }

    const failures: Array<{
      index: number;
      expected: Partial<Tab>;
      actual: Tab;
    }> = [];

    expected.forEach((partial, index) => {
      const matches = Object.entries(partial).every(
        ([key, value]) =>
          JSON.stringify(tabs[index][key as keyof Tab]) ===
          JSON.stringify(value),
      );

      if (!matches) {
        failures.push({
          index,
          expected: partial,
          actual: tabs[index],
        });
      }
    });

    const pass = failures.length === 0;

    return {
      pass,
      message: () =>
        pass
          ? `Expected tabs not to match the provided order`
          : `Expected tabs to match order, but found mismatches:\n${failures
              .map(
                (f) =>
                  `  [${f.index}] expected ${JSON.stringify(f.expected)}, got ${JSON.stringify(f.actual)}`,
              )
              .join("\n")}`,
      actual: tabs,
      expected,
    };
  },
});
