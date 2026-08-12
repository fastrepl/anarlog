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
  removeDraft: vi.fn(),
  removeStarterDraft: vi.fn(),
  selection: null as unknown,
  setSettingValue: vi.fn(() => Promise.resolve()),
  toastError: vi.fn(),
  toastSuccess: vi.fn(),
}));

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => mocks.billing,
}));

vi.mock("~/automations/actions", () => ({
  useRemoveStarterDraft: () => ({ mutate: mocks.removeStarterDraft }),
  useDeleteChatAutomation: () => ({ mutate: mocks.deleteChatAutomation }),
}));

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
    mocks.removeDraft.mockClear();
    mocks.removeStarterDraft.mockClear();
    mocks.selection = null;
    mocks.setSettingValue.mockClear();
    mocks.toastError.mockClear();
    mocks.toastSuccess.mockClear();
  });

  it("shows the overview when nothing is selected", () => {
    renderAutomations();

    expect(screen.getByRole("heading", { name: "Automations" })).toBeTruthy();
    expect(screen.getByText("No automation draft yet")).toBeTruthy();
    expect(
      screen.getByText(
        "Choose a starter from the sidebar or describe an automation in Chat.",
      ),
    ).toBeTruthy();
  });

  it("shows the untitled draft page after the sidebar plus button", () => {
    mocks.selection = { kind: "draft", draftId: "draft-1" };

    renderAutomations();

    expect(
      screen.getByRole("heading", { name: "Untitled automation" }),
    ).toBeTruthy();
    expect(screen.getByText("Start in Chat")).toBeTruthy();
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

  it("offers the Pro upgrade instead of saving on the free plan", () => {
    mocks.billing.isPro = false;
    mocks.selection = { kind: "starter", starterId: "notion-project-notes" };

    renderAutomations();

    fireEvent.click(screen.getByRole("button", { name: "Upgrade to save" }));

    expect(mocks.billing.upgradeToPro).toHaveBeenCalledOnce();
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
    expect(screen.getByText("Drafted in Chat")).toBeTruthy();
    expect(screen.getByText("Draft")).toBeTruthy();
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
