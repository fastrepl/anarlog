import { useLingui } from "@lingui/react/macro";
import { useEffect, useRef } from "react";

import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { useMyWorkspaceInvitations } from "./my-invitations";

import { useTabs } from "~/store/zustand/tabs";

export function WorkspaceInvitationToasts() {
  const { t } = useLingui();
  const invitations = useMyWorkspaceInvitations();
  const openNew = useTabs((state) => state.openNew);
  const toastedIds = useRef(new Set<string>());

  useEffect(() => {
    const current = new Set(
      (invitations.data ?? []).map((invitation) => invitation.invitationId),
    );

    for (const id of toastedIds.current) {
      if (!current.has(id)) {
        sonnerToast.dismiss(`team-invitation:${id}`);
        toastedIds.current.delete(id);
      }
    }

    for (const invitation of invitations.data ?? []) {
      if (toastedIds.current.has(invitation.invitationId)) continue;
      toastedIds.current.add(invitation.invitationId);
      const toastId = `team-invitation:${invitation.invitationId}`;
      sonnerToast(t`You've been invited to join ${invitation.workspaceName}`, {
        id: toastId,
        duration: Infinity,
        description: invitation.invitedByEmail
          ? t`Invited by ${invitation.invitedByEmail}`
          : undefined,
        action: {
          label: t`View`,
          onClick: () => {
            openNew({ type: "settings", state: { tab: "team" } });
            sonnerToast.dismiss(toastId);
          },
        },
      });
    }
  }, [invitations.data, openNew, t]);

  return null;
}
