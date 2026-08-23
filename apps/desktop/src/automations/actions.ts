import { useLingui } from "@lingui/react/macro";
import { useMutation } from "@tanstack/react-query";

import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { useAutomationSelection } from "./selection";
import { STARTER_AUTOMATIONS, type StarterId } from "./starters";
import { parseAutomationWorkflows, saveAutomationWorkflows } from "./workflows";

import { deleteChatGroup } from "~/chat/store/queries";
import { getStoredSettingValues, setSettingValues } from "~/settings/queries";
import type { SettingValues } from "~/settings/schema";

export function useRemoveStarterDraft() {
  const { t } = useLingui();
  const clearSelection = useAutomationSelection(
    (state) => state.clearSelection,
  );

  return useMutation({
    mutationKey: ["automation-remove-starter-draft"],
    mutationFn: async (starterId: StarterId) => {
      const stored = await getStoredSettingValues();
      const updates: SettingValues = {};
      updates[STARTER_AUTOMATIONS[starterId].enabledKey] = false;
      if (stored.values.automation_draft_template === starterId) {
        updates.automation_draft_template = "";
      }
      await setSettingValues(updates);
    },
    onSuccess: (_, starterId) => {
      clearSelection({ kind: "starter", starterId });
      sonnerToast.success(t`Automation removed`);
    },
    onError: () => sonnerToast.error(t`Could not remove the automation`),
  });
}

export function useDeleteChatAutomation() {
  const { t } = useLingui();
  const clearSelection = useAutomationSelection(
    (state) => state.clearSelection,
  );

  return useMutation({
    mutationKey: ["automation-delete-chat"],
    mutationFn: async (groupId: string) => {
      const stored = await getStoredSettingValues();
      const workflows = parseAutomationWorkflows(
        stored.values.automation_workflows,
      );
      const remaining = workflows.filter(
        (workflow) => workflow.chatGroupId !== groupId,
      );
      if (remaining.length !== workflows.length) {
        await saveAutomationWorkflows(remaining);
      }
      await deleteChatGroup(groupId);
    },
    onSuccess: (_, groupId) => {
      clearSelection({ kind: "chat", groupId });
      sonnerToast.success(t`Automation deleted`);
    },
    onError: () => sonnerToast.error(t`Could not delete the automation`),
  });
}

export function useDeleteWorkflow() {
  const { t } = useLingui();
  const clearSelection = useAutomationSelection(
    (state) => state.clearSelection,
  );

  return useMutation({
    mutationKey: ["automation-delete-workflow"],
    mutationFn: async (workflowId: string) => {
      const stored = await getStoredSettingValues();
      const workflows = parseAutomationWorkflows(
        stored.values.automation_workflows,
      );
      const workflow = workflows.find((item) => item.id === workflowId);
      await saveAutomationWorkflows(
        workflows.filter((item) => item.id !== workflowId),
      );
      if (workflow?.chatGroupId) {
        await deleteChatGroup(workflow.chatGroupId);
      }
    },
    onSuccess: (_, workflowId) => {
      clearSelection({ kind: "workflow", workflowId });
      sonnerToast.success(t`Automation deleted`);
    },
    onError: () => sonnerToast.error(t`Could not delete the automation`),
  });
}
