import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { createElement } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@iconify-icon/react", () => ({
  Icon: (props: Record<string, unknown>) =>
    createElement("iconify-icon", props),
}));

type ContextMenuItem =
  | { id: string; text: string; action: () => void }
  | { separator: true };

const mocks = vi.hoisted(() => ({
  automations: [] as Array<{
    id: string;
    ownerUserId: string;
    title: string;
    createdAt: string;
    updatedAt: string;
  }>,
  contextMenus: [] as Array<
    Array<{ id: string; text: string; action: () => void }>
  >,
  deleteChatAutomation: vi.fn(),
  draftIds: [] as string[],
  removeDraft: vi.fn(),
  removeStarterDraft: vi.fn(),
  selection: null as unknown,
  selectStarter: vi.fn(),
  selectChatAutomation: vi.fn(),
  selectDraft: vi.fn(),
  selectWorkflow: vi.fn(),
  startDraft: vi.fn(),
  saveAutomationWorkflows: vi.fn<
    (workflows: Array<{ id: string }>) => Promise<void>
  >(() => Promise.resolve()),
  showContextMenu: vi.fn(),
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
}));

vi.mock("~/chat/store/queries", () => ({
  useChatGroups: () => mocks.automations,
}));

vi.mock("~/automations/actions", () => ({
  useRemoveStarterDraft: () => ({ mutate: mocks.removeStarterDraft }),
  useDeleteChatAutomation: () => ({ mutate: mocks.deleteChatAutomation }),
  useDeleteWorkflow: () => ({ mutate: vi.fn() }),
}));

vi.mock("~/automations/workflows", async (importOriginal) => {
  const actual =
    await importOriginal<typeof import("~/automations/workflows")>();
  return {
    ...actual,
    useAutomationWorkflows: () => mocks.workflows,
    saveAutomationWorkflows: mocks.saveAutomationWorkflows,
  };
});

vi.mock("~/automations/selection", () => ({
  useAutomationSelection: (selector: (state: unknown) => unknown) =>
    selector({
      draftIds: mocks.draftIds,
      selectStarter: mocks.selectStarter,
      selectChatAutomation: mocks.selectChatAutomation,
      selectDraft: mocks.selectDraft,
      selectWorkflow: mocks.selectWorkflow,
      startDraft: mocks.startDraft,
      removeDraft: mocks.removeDraft,
    }),
  useEffectiveAutomationSelection: () => mocks.selection,
}));

vi.mock("~/shared/hooks/useNativeContextMenu", () => ({
  useNativeContextMenu: (items: ContextMenuItem[]) => {
    mocks.contextMenus.push(
      items.filter(
        (item): item is Exclude<ContextMenuItem, { separator: true }> =>
          !("separator" in item),
      ),
    );
    return mocks.showContextMenu;
  },
}));

vi.mock("~/sidebar/custom-sidebar-header", () => ({
  CustomSidebarHeader: ({ children }: { children?: React.ReactNode }) => (
    <header>{children}</header>
  ),
}));

import { AutomationsNav } from "./automations";

function findContextMenuItem(id: string) {
  for (const items of mocks.contextMenus) {
    const match = items.find((item) => item.id === id);
    if (match) {
      return match;
    }
  }
  return undefined;
}

describe("AutomationsNav", () => {
  afterEach(cleanup);

  beforeEach(() => {
    mocks.automations = [
      {
        id: "automation-1",
        ownerUserId: "user-1",
        title: "Share weekly recap",
        createdAt: "2026-08-03T10:00:00.000Z",
        updatedAt: "2026-08-03T10:00:00.000Z",
      },
      {
        id: "automation-2",
        ownerUserId: "user-1",
        title: "Update project notes",
        createdAt: "2026-08-02T10:00:00.000Z",
        updatedAt: "2026-08-02T10:00:00.000Z",
      },
    ];
    mocks.contextMenus = [];
    mocks.draftIds = [];
    mocks.selection = { kind: "chat", groupId: "automation-1" };
    mocks.deleteChatAutomation.mockClear();
    mocks.removeDraft.mockClear();
    mocks.removeStarterDraft.mockClear();
    mocks.selectStarter.mockClear();
    mocks.selectChatAutomation.mockClear();
    mocks.selectDraft.mockClear();
    mocks.selectWorkflow.mockClear();
    mocks.startDraft.mockClear();
    mocks.saveAutomationWorkflows.mockClear();
    mocks.workflows = [];
    mocks.showContextMenu.mockClear();
  });

  it("lists the starter automations and selects one", () => {
    render(<AutomationsNav />);

    expect(screen.getByText("Get started")).toBeTruthy();
    expect(screen.getByText("Share a meeting recap in Slack")).toBeTruthy();
    expect(screen.getByText("Update project notes in Notion")).toBeTruthy();
    expect(
      screen.getByText("Turn action items into Linear issues"),
    ).toBeTruthy();
    expect(screen.getByText("Export every meeting as Markdown")).toBeTruthy();

    fireEvent.click(
      screen.getByRole("button", { name: /Share a meeting recap in Slack/ }),
    );

    expect(mocks.selectStarter).toHaveBeenCalledWith("slack-recap");
  });

  it("marks the selected starter", () => {
    mocks.selection = { kind: "starter", starterId: "markdown-export" };

    render(<AutomationsNav />);

    expect(
      screen
        .getByRole("button", { name: /Export every meeting as Markdown/ })
        .getAttribute("aria-current"),
    ).toBe("page");
  });

  it("lists chat automations and opens the selected conversation", () => {
    render(<AutomationsNav />);

    expect(screen.getByText("My automations")).toBeTruthy();
    expect(
      screen
        .getByRole("button", { name: /Share weekly recap/ })
        .getAttribute("aria-current"),
    ).toBe("page");

    const chatAutomationButton = screen
      .getByText("Update project notes")
      .closest("button");
    expect(chatAutomationButton).toBeTruthy();
    fireEvent.click(chatAutomationButton!);

    expect(mocks.selectChatAutomation).toHaveBeenCalledWith("automation-2");
  });

  it("offers edit and remove in the starter context menu", () => {
    render(<AutomationsNav />);

    fireEvent.contextMenu(
      screen.getByRole("button", { name: /Share a meeting recap in Slack/ }),
    );

    expect(mocks.selectStarter).toHaveBeenCalledWith("slack-recap");
    expect(mocks.showContextMenu).toHaveBeenCalled();

    findContextMenuItem("remove-automation-slack-recap")?.action();
    expect(mocks.removeStarterDraft).toHaveBeenCalledWith("slack-recap");
  });

  it("offers edit and delete in the chat automation context menu", () => {
    render(<AutomationsNav />);

    const chatAutomationButton = screen
      .getByText("Update project notes")
      .closest("button");
    fireEvent.contextMenu(chatAutomationButton!);

    expect(mocks.selectChatAutomation).toHaveBeenCalledWith("automation-2");
    expect(mocks.showContextMenu).toHaveBeenCalled();

    findContextMenuItem("delete-automation-automation-2")?.action();
    expect(mocks.deleteChatAutomation).toHaveBeenCalledWith("automation-2");
  });

  it("filters starters and chat automations together", () => {
    render(<AutomationsNav />);

    fireEvent.change(screen.getByPlaceholderText("Search automations..."), {
      target: { value: "project" },
    });

    expect(screen.queryByText("Share a meeting recap in Slack")).toBeNull();
    expect(screen.getByText("Update project notes in Notion")).toBeTruthy();
    expect(screen.queryByText("Share weekly recap")).toBeNull();
    expect(screen.getByText("Update project notes")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Clear search" }));

    expect(screen.getByText("Share weekly recap")).toBeTruthy();
  });

  it("shows an empty state when the search matches nothing", () => {
    render(<AutomationsNav />);

    fireEvent.change(screen.getByPlaceholderText("Search automations..."), {
      target: { value: "zzz" },
    });

    expect(screen.getByText("No automations found")).toBeTruthy();
  });

  it("starts a new automation workflow from the sidebar", async () => {
    render(<AutomationsNav />);

    fireEvent.click(screen.getByRole("button", { name: "New automation" }));

    expect(mocks.saveAutomationWorkflows).toHaveBeenCalledOnce();
    const saved = mocks.saveAutomationWorkflows.mock.calls[0]?.[0];
    expect(saved).toHaveLength(1);
    await vi.waitFor(() => {
      expect(mocks.selectWorkflow).toHaveBeenCalledWith(saved[0]?.id);
    });
  });

  it("shows drafts as untitled automations and selects them", () => {
    mocks.draftIds = ["draft-1"];
    mocks.selection = { kind: "draft", draftId: "draft-1" };

    render(<AutomationsNav />);

    const draftButton = screen
      .getByText("Untitled automation")
      .closest("button");
    expect(draftButton).toBeTruthy();
    expect(draftButton!.getAttribute("aria-current")).toBe("page");
    expect(screen.getByText("Draft")).toBeTruthy();

    fireEvent.click(draftButton!);

    expect(mocks.selectDraft).toHaveBeenCalledWith("draft-1");
  });

  it("offers delete in the draft context menu", () => {
    mocks.draftIds = ["draft-1"];

    render(<AutomationsNav />);

    fireEvent.contextMenu(
      screen.getByText("Untitled automation").closest("button")!,
    );

    expect(mocks.selectDraft).toHaveBeenCalledWith("draft-1");
    expect(mocks.showContextMenu).toHaveBeenCalled();

    findContextMenuItem("delete-automation-draft-1")?.action();
    expect(mocks.removeDraft).toHaveBeenCalledWith("draft-1");
  });
});
