import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { createElement } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// The real iconify-icon web component renders asynchronously via timers that
// can fire after the test environment is torn down ("document is not
// defined" unhandled errors), so render an inert element instead.
vi.mock("@iconify-icon/react", () => ({
  Icon: (props: Record<string, unknown>) =>
    createElement("iconify-icon", props),
}));

const mocks = vi.hoisted(() => ({
  billing: {
    isPro: true,
    isReady: true,
    upgradeToPro: vi.fn(),
  },
  chatGroup: null as {
    id: string;
    ownerUserId: string;
    title: string;
    createdAt: string;
    updatedAt: string;
  } | null,
  deleteChatAutomation: vi.fn(),
  deleteWorkflow: vi.fn(),
  removeDraft: vi.fn(),
  removeStarterDraft: vi.fn(),
  selection: null as unknown,
  setSettingValue: vi.fn(() => Promise.resolve()),
  workflows: [] as Array<{
    id: string;
    title: string;
    enabled: boolean;
    trigger: string;
    steps: unknown[];
    lastRun: null;
    processedSessionIds: string[];
    chatGroupId: string | null;
  }>,
  toastError: vi.fn(),
  toastSuccess: vi.fn(),
  toastWarning: vi.fn(),
}));

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => mocks.billing,
}));

vi.mock("~/automations/actions", () => ({
  useRemoveStarterDraft: () => ({ mutate: mocks.removeStarterDraft }),
  useDeleteChatAutomation: () => ({ mutate: mocks.deleteChatAutomation }),
  useDeleteWorkflow: () => ({ mutate: mocks.deleteWorkflow }),
}));

vi.mock("~/automations/workflows", async (importOriginal) => {
  const actual =
    await importOriginal<typeof import("~/automations/workflows")>();
  return {
    ...actual,
    useAutomationWorkflows: () => mocks.workflows,
    saveAutomationWorkflows: vi.fn(() => Promise.resolve()),
  };
});

vi.mock("~/automations/selection", () => ({
  useAutomationSelection: (selector: (state: unknown) => unknown) =>
    selector({ removeDraft: mocks.removeDraft }),
  useEffectiveAutomationSelection: () => mocks.selection,
}));

vi.mock("~/chat/store/queries", () => ({
  useChatGroup: () => mocks.chatGroup,
}));

vi.mock("~/settings/queries", () => ({
  setSettingValue: mocks.setSettingValue,
  setSettingValues: mocks.setSettingValue,
  getStoredSettingValues: () =>
    Promise.resolve({
      values: { automation_workflows: "[]" },
      hasValues: new Set(["automation_workflows"]),
    }),
  useStoredSettingValue: () => ({ value: "", hasValue: false }),
  useStoredSettingValues: () => ({ values: {}, hasValues: new Set() }),
}));

vi.mock("./starter-config", () => ({
  AutomationLastRunLine: () => null,
  MarkdownExportConfig: () => <div data-testid="config-markdown" />,
  SlackRecapConfig: () => <div data-testid="config-slack" />,
  LinearIssuesConfig: () => <div data-testid="config-linear" />,
  NotionUpdateConfig: () => <div data-testid="config-notion" />,
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  sonnerToast: {
    error: mocks.toastError,
    success: mocks.toastSuccess,
    warning: mocks.toastWarning,
  },
}));

import { AutomationsContent } from ".";

function renderAutomations() {
  const queryClient = new QueryClient({
    defaultOptions: { mutations: { retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <AutomationsContent />
    </QueryClientProvider>,
  );
}

describe("AutomationsContent", () => {
  // Radix's focus scope schedules a focus dispatch on unmount, so drain the
  // timer queue while jsdom is still alive; left pending it fires against a
  // torn-down realm and vitest reports an unhandled error.
  afterEach(async () => {
    cleanup();
    await new Promise((resolve) => setTimeout(resolve, 0));
  });

  beforeEach(() => {
    mocks.billing.isPro = true;
    mocks.billing.isReady = true;
    mocks.billing.upgradeToPro.mockClear();
    mocks.chatGroup = null;
    mocks.deleteChatAutomation.mockClear();
    mocks.deleteWorkflow.mockClear();
    mocks.removeDraft.mockClear();
    mocks.removeStarterDraft.mockClear();
    mocks.selection = null;
    mocks.workflows = [];
    mocks.setSettingValue.mockClear();
    mocks.toastError.mockClear();
    mocks.toastSuccess.mockClear();
    mocks.toastWarning.mockClear();
  });

  it("shows the overview when nothing is selected", () => {
    renderAutomations();

    expect(screen.getByRole("heading", { name: "Automations" })).toBeTruthy();
    expect(screen.getByText("No automation draft yet")).toBeTruthy();
    expect(
      screen.getByText(
        "Choose a starter from the sidebar, or create a workflow and add steps like Zapier.",
      ),
    ).toBeTruthy();
  });

  it("shows the untitled draft page after the sidebar plus button", () => {
    mocks.selection = { kind: "draft", draftId: "draft-1" };

    renderAutomations();

    expect(
      screen.getByRole("heading", { name: "Untitled automation" }),
    ).toBeTruthy();
    expect(
      screen.getByText("Add a trigger, then stack actions like Zapier."),
    ).toBeTruthy();
    expect(screen.getByText("Add an action")).toBeTruthy();
  });

  it("deletes a draft from its actions menu", async () => {
    mocks.selection = { kind: "draft", draftId: "draft-1" };

    renderAutomations();

    const trigger = screen.getByRole("button", { name: "Automation actions" });
    fireEvent.pointerDown(trigger);
    fireEvent.click(trigger);

    fireEvent.click(await screen.findByText("Delete automation"));

    expect(mocks.removeDraft).toHaveBeenCalledWith("draft-1");
  });

  it("shows the selected starter as an inspectable deterministic draft", () => {
    mocks.selection = { kind: "starter", starterId: "slack-recap" };

    renderAutomations();

    expect(screen.getByText("Use the AI meeting summary")).toBeTruthy();
    expect(screen.getByText("Post to a channel")).toBeTruthy();
    expect(screen.getByTestId("config-slack")).toBeTruthy();
    expect(
      screen.getByRole<HTMLButtonElement>("button", { name: "Test" }).disabled,
    ).toBe(true);
    expect(
      screen.getByRole<HTMLButtonElement>("button", {
        name: "Save & enable",
      }).disabled,
    ).toBe(true);

    fireEvent.click(screen.getByRole("button", { name: "Preview" }));

    expect(screen.getByText("Expected output")).toBeTruthy();
    expect(
      screen.getByText(/A Slack message with the meeting title and recap/),
    ).toBeTruthy();
  });

  it("uses product marks without icon tiles", () => {
    mocks.selection = { kind: "starter", starterId: "slack-recap" };

    const { container } = renderAutomations();

    const header = screen
      .getByRole("heading", {
        level: 2,
        name: "Share a meeting recap in Slack",
      })
      .closest("header");
    const slackIcon = container.querySelector(
      'iconify-icon[icon="logos:slack-icon"]',
    );

    expect(header).toBeTruthy();
    expect(slackIcon).toBeTruthy();
    expect(slackIcon?.closest("header")).toBe(header);
    expect(
      screen
        .getByRole("button", { name: "Automation actions" })
        .closest("header"),
    ).toBe(header);
    expect(slackIcon?.parentElement?.className).not.toContain("bg-muted");
    expect(slackIcon?.parentElement?.className).not.toContain("rounded");
  });

  it("matches the templates header and body gutters", () => {
    mocks.selection = { kind: "starter", starterId: "slack-recap" };

    renderAutomations();

    const header = screen
      .getByRole("heading", {
        level: 2,
        name: "Share a meeting recap in Slack",
      })
      .closest("header");
    const body = header?.nextElementSibling;

    expect(header?.className).toContain("h-12");
    expect(header?.className).toContain("pl-3");
    expect(header?.className).toContain("pr-1");
    expect(body?.className).toContain("px-6");
    expect(body?.className).toContain("pt-3");
  });

  it("saves the selected draft for Pro users", async () => {
    mocks.selection = { kind: "starter", starterId: "markdown-export" };

    renderAutomations();

    fireEvent.click(screen.getByRole("button", { name: "Save draft" }));

    await waitFor(() => {
      expect(mocks.setSettingValue).toHaveBeenCalledWith(
        "automation_draft_template",
        "markdown-export",
      );
    });
    expect(mocks.toastSuccess).toHaveBeenCalledWith("Automation draft saved");
  });

  it("toasts instead of saving on the free plan", () => {
    mocks.billing.isPro = false;
    mocks.selection = { kind: "starter", starterId: "notion-project-notes" };

    renderAutomations();

    fireEvent.click(screen.getByRole("button", { name: "Save draft" }));

    expect(mocks.toastWarning).toHaveBeenCalledWith(
      "This requires Anarlog Pro",
      {
        action: {
          label: "Upgrade",
          onClick: expect.any(Function),
        },
      },
    );
    expect(mocks.billing.upgradeToPro).not.toHaveBeenCalled();
    expect(mocks.setSettingValue).not.toHaveBeenCalled();
  });

  it("shows a dedicated view for a chat-created automation", () => {
    mocks.selection = { kind: "chat", groupId: "automation-1" };
    mocks.chatGroup = {
      id: "automation-1",
      ownerUserId: "user-1",
      title: "Share weekly recap",
      createdAt: "2026-08-03T10:00:00.000Z",
      updatedAt: "2026-08-03T10:00:00.000Z",
    };

    renderAutomations();

    expect(
      screen.getByRole("heading", { name: "Share weekly recap" }),
    ).toBeTruthy();
    expect(
      screen.getByText("Add a trigger, then stack actions like Zapier."),
    ).toBeTruthy();
    expect(screen.getByText("Add an action")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Save & enable" })).toBeTruthy();
  });

  it("removes the starter automation from the actions menu", async () => {
    mocks.selection = { kind: "starter", starterId: "slack-recap" };

    renderAutomations();

    const trigger = screen.getByRole("button", { name: "Automation actions" });
    fireEvent.pointerDown(trigger);
    fireEvent.click(trigger);

    fireEvent.click(await screen.findByText("Remove automation"));

    expect(mocks.removeStarterDraft).toHaveBeenCalledWith("slack-recap");
  });

  it("deletes a chat automation from the actions menu", async () => {
    mocks.selection = { kind: "chat", groupId: "automation-1" };
    mocks.chatGroup = {
      id: "automation-1",
      ownerUserId: "user-1",
      title: "Share weekly recap",
      createdAt: "2026-08-03T10:00:00.000Z",
      updatedAt: "2026-08-03T10:00:00.000Z",
    };

    renderAutomations();

    const trigger = screen.getByRole("button", { name: "Automation actions" });
    fireEvent.pointerDown(trigger);
    fireEvent.click(trigger);

    fireEvent.click(await screen.findByText("Delete automation"));

    expect(mocks.deleteChatAutomation).toHaveBeenCalledWith("automation-1");
  });
});
