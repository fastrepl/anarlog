import { useLingui } from "@lingui/react/macro";
import { useMutation } from "@tanstack/react-query";

import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { useAutomationSelection } from "./selection";
import { STARTER_AUTOMATIONS, type StarterId } from "./starters";

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
    mutationFn: (groupId: string) => deleteChatGroup(groupId),
    onSuccess: (_, groupId) => {
      clearSelection({ kind: "chat", groupId });
      sonnerToast.success(t`Automation deleted`);
    },
    onError: () => sonnerToast.error(t`Could not delete the automation`),
  });
}
