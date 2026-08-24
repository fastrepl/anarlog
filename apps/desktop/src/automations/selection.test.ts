import { renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const settingsMocks = vi.hoisted(() => ({
  storedDraft: undefined as string | undefined,
  workflows: "[]",
}));

vi.mock("~/settings/queries", () => ({
  useStoredSettingValue: () => ({
    value: settingsMocks.storedDraft,
    hasValue: Boolean(settingsMocks.storedDraft),
  }),
  getStoredSettingValues: () =>
    Promise.resolve({
      values: { automation_workflows: settingsMocks.workflows },
      hasValues: new Set(["automation_workflows"]),
    }),
  setSettingValue: () => Promise.resolve(),
}));

import {
  useAutomationSelection,
  useEffectiveAutomationSelection,
} from "./selection";

import { useChatContext } from "~/chat/state/chat-context";

function automationsChat() {
  return useChatContext.getState().chatByScope.automations;
}

describe("useAutomationSelection", () => {
  beforeEach(() => {
    settingsMocks.storedDraft = undefined;
    settingsMocks.workflows = "[]";
    useAutomationSelection.setState({
      selection: null,
      draftIds: [],
      chatBySelection: {},
    });
    useChatContext.setState({
      chatByScope: {
        general: { groupId: undefined, sessionId: "general-session" },
        automations: { groupId: undefined, sessionId: "initial-session" },
      },
    });
  });

  it("opens the persisted thread when selecting a chat automation", () => {
    useAutomationSelection.getState().selectChatAutomation("group-1");

    expect(useAutomationSelection.getState().selection).toEqual({
      kind: "chat",
      groupId: "group-1",
    });
    expect(automationsChat()).toEqual({
      groupId: "group-1",
      sessionId: "group-1",
    });
  });

  it("keeps a chat thread per automation across selection switches", () => {
    const { selectStarter } = useAutomationSelection.getState();

    selectStarter("slack-recap");
    const slackSession = automationsChat().sessionId;

    // Chatting creates a group for the live automations chat.
    useChatContext.getState().setGroupId("automations", "slack-group");

    selectStarter("markdown-export");
    expect(automationsChat().groupId).toBeUndefined();
    expect(automationsChat().sessionId).not.toBe(slackSession);

    selectStarter("slack-recap");
    expect(automationsChat()).toEqual({
      groupId: "slack-group",
      sessionId: slackSession,
    });
  });

  it("clears the current selection and its stored chat thread", () => {
    const { selectStarter, clearSelection } = useAutomationSelection.getState();

    selectStarter("slack-recap");
    useChatContext.getState().setGroupId("automations", "slack-group");
    const slackSession = automationsChat().sessionId;

    clearSelection({ kind: "starter", starterId: "slack-recap" });

    expect(useAutomationSelection.getState().selection).toBeNull();
    expect(useAutomationSelection.getState().chatBySelection).toEqual({});
    expect(automationsChat().groupId).toBeUndefined();
    expect(automationsChat().sessionId).not.toBe(slackSession);
  });

  it("keeps the current selection when clearing another automation", () => {
    const { selectStarter, clearSelection } = useAutomationSelection.getState();

    selectStarter("slack-recap");
    const slackChat = automationsChat();

    clearSelection({ kind: "chat", groupId: "other-group" });

    expect(useAutomationSelection.getState().selection).toEqual({
      kind: "starter",
      starterId: "slack-recap",
    });
    expect(automationsChat()).toEqual(slackChat);
  });

  it("opens a persisted workflow chat thread when selecting after reload", () => {
    useAutomationSelection.getState().selectWorkflow("wf-1", "workflow-group");

    expect(useAutomationSelection.getState().selection).toEqual({
      kind: "workflow",
      workflowId: "wf-1",
    });
    expect(automationsChat()).toEqual({
      groupId: "workflow-group",
      sessionId: "workflow-group",
    });
  });

  it("restores a persisted workflow chat when the group id is only in settings", async () => {
    settingsMocks.workflows = JSON.stringify([
      {
        id: "wf-1",
        title: "Recap",
        enabled: true,
        trigger: "note_enhanced",
        steps: [],
        lastRun: null,
        processedSessionIds: [],
        chatGroupId: "workflow-group",
      },
    ]);

    useAutomationSelection.getState().selectWorkflow("wf-1");

    expect(automationsChat().groupId).toBeUndefined();
    await waitFor(() => {
      expect(automationsChat()).toEqual({
        groupId: "workflow-group",
        sessionId: "workflow-group",
      });
    });
  });

  it("prefers the in-memory workflow chat snapshot over the persisted group", () => {
    useAutomationSelection.getState().selectWorkflow("wf-1");
    const liveSession = automationsChat().sessionId;
    useChatContext.getState().setGroupId("automations", "live-group");

    useAutomationSelection.getState().selectStarter("slack-recap");
    useAutomationSelection.getState().selectWorkflow("wf-1", "persisted-group");

    expect(automationsChat()).toEqual({
      groupId: "live-group",
      sessionId: liveSession,
    });
  });

  it("creates and selects a visible draft with a fresh chat", () => {
    useAutomationSelection.getState().startDraft();

    const { selection, draftIds } = useAutomationSelection.getState();
    expect(draftIds).toHaveLength(1);
    expect(selection).toEqual({ kind: "draft", draftId: draftIds[0] });
    expect(automationsChat().groupId).toBeUndefined();

    useAutomationSelection.getState().startDraft();

    const next = useAutomationSelection.getState();
    expect(next.draftIds).toHaveLength(2);
    expect(next.selection).toEqual({
      kind: "draft",
      draftId: next.draftIds[0],
    });
  });

  it("converts a draft into a chat automation once its chat has a group", () => {
    useAutomationSelection.getState().startDraft();

    useChatContext.getState().setGroupId("automations", "created-group");

    const { selection, draftIds } = useAutomationSelection.getState();
    expect(selection).toEqual({ kind: "chat", groupId: "created-group" });
    expect(draftIds).toHaveLength(0);
  });

  it("removes a draft and falls back to the overview", () => {
    useAutomationSelection.getState().startDraft();
    const draftId = useAutomationSelection.getState().draftIds[0]!;
    const draftSession = automationsChat().sessionId;

    useAutomationSelection.getState().removeDraft(draftId);

    const { selection, draftIds } = useAutomationSelection.getState();
    expect(selection).toBeNull();
    expect(draftIds).toHaveLength(0);
    expect(automationsChat().sessionId).not.toBe(draftSession);
  });
});

describe("useEffectiveAutomationSelection", () => {
  beforeEach(() => {
    settingsMocks.storedDraft = undefined;
    useAutomationSelection.setState({ selection: null, chatBySelection: {} });
  });

  it("falls back to the stored draft starter", () => {
    settingsMocks.storedDraft = "slack-recap";

    const { result } = renderHook(() => useEffectiveAutomationSelection());

    expect(result.current).toEqual({
      kind: "starter",
      starterId: "slack-recap",
    });
  });

  it("prefers the explicit selection over the stored draft", () => {
    settingsMocks.storedDraft = "slack-recap";
    useAutomationSelection.setState({
      selection: { kind: "draft", draftId: "draft-1" },
    });

    const { result } = renderHook(() => useEffectiveAutomationSelection());

    expect(result.current).toEqual({ kind: "draft", draftId: "draft-1" });
  });

  it("returns null without a selection or a valid stored draft", () => {
    settingsMocks.storedDraft = "not-a-starter";

    const { result } = renderHook(() => useEffectiveAutomationSelection());

    expect(result.current).toBeNull();
  });
});
