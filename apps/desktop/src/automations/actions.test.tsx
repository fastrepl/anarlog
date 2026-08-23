import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  clearSelection: vi.fn(),
  deleteChatGroup: vi.fn(() => Promise.resolve()),
  setSettingValues: vi.fn(() => Promise.resolve()),
  storedValues: {} as Record<string, string>,
  toastError: vi.fn(),
  toastSuccess: vi.fn(),
}));

vi.mock("./selection", () => ({
  useAutomationSelection: (selector: (state: unknown) => unknown) =>
    selector({ clearSelection: mocks.clearSelection }),
}));

vi.mock("~/chat/store/queries", () => ({
  deleteChatGroup: mocks.deleteChatGroup,
}));

vi.mock("~/settings/queries", () => ({
  getStoredSettingValues: () =>
    Promise.resolve({
      values: mocks.storedValues,
      hasValues: new Set(Object.keys(mocks.storedValues)),
    }),
  setSettingValues: mocks.setSettingValues,
  setSettingValue: () => Promise.resolve(),
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  sonnerToast: {
    error: mocks.toastError,
    success: mocks.toastSuccess,
  },
}));

import { useDeleteChatAutomation, useRemoveStarterDraft } from "./actions";

function wrapper({ children }: { children: ReactNode }) {
  const queryClient = new QueryClient({
    defaultOptions: { mutations: { retry: false } },
  });

  return (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}

describe("useRemoveStarterDraft", () => {
  beforeEach(() => {
    mocks.clearSelection.mockClear();
    mocks.setSettingValues.mockClear();
    mocks.storedValues = {};
    mocks.toastError.mockClear();
    mocks.toastSuccess.mockClear();
  });

  it("disables the starter and clears its stored draft", async () => {
    mocks.storedValues = { automation_draft_template: "slack-recap" };

    const { result } = renderHook(() => useRemoveStarterDraft(), { wrapper });
    result.current.mutate("slack-recap");

    await waitFor(() => {
      expect(mocks.setSettingValues).toHaveBeenCalledWith({
        automation_slack_recap_enabled: false,
        automation_draft_template: "",
      });
    });
    expect(mocks.clearSelection).toHaveBeenCalledWith({
      kind: "starter",
      starterId: "slack-recap",
    });
    expect(mocks.toastSuccess).toHaveBeenCalledWith("Automation removed");
  });

  it("keeps another starter's stored draft", async () => {
    mocks.storedValues = { automation_draft_template: "markdown-export" };

    const { result } = renderHook(() => useRemoveStarterDraft(), { wrapper });
    result.current.mutate("slack-recap");

    await waitFor(() => {
      expect(mocks.setSettingValues).toHaveBeenCalledWith({
        automation_slack_recap_enabled: false,
      });
    });
  });
});

describe("useDeleteChatAutomation", () => {
  beforeEach(() => {
    mocks.clearSelection.mockClear();
    mocks.deleteChatGroup.mockClear();
    mocks.toastError.mockClear();
    mocks.toastSuccess.mockClear();
  });

  it("deletes the chat group and clears the selection", async () => {
    const { result } = renderHook(() => useDeleteChatAutomation(), { wrapper });
    result.current.mutate("group-1");

    await waitFor(() => {
      expect(mocks.deleteChatGroup).toHaveBeenCalledWith("group-1");
    });
    expect(mocks.clearSelection).toHaveBeenCalledWith({
      kind: "chat",
      groupId: "group-1",
    });
    expect(mocks.toastSuccess).toHaveBeenCalledWith("Automation deleted");
  });
});
