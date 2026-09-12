import { useLingui } from "@lingui/react/macro";

import { commands as notificationCommands } from "@anlg/plugin-notification";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import type { MyWorkspaceInvitation } from "./client";
import { useMyWorkspaceInvitations } from "./my-invitations";

import { useConfigValue } from "~/shared/config";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { isAppWindowInactive } from "~/shared/window-activity";
import { useTabs } from "~/store/zustand/tabs";

export function WorkspaceInvitationToasts() {
  const invitations = useMyWorkspaceInvitations();

  return invitations.data?.map((invitation) => (
    <WorkspaceInvitationToast
      key={invitation.invitationId}
      invitation={invitation}
    />
  ));
}

function WorkspaceInvitationToast({
  invitation,
}: {
  invitation: MyWorkspaceInvitation;
}) {
  const { t } = useLingui();
  const openNew = useTabs((state) => state.openNew);
  const notificationsDisabled = useConfigValue("notification_disabled");

  useMountEffect(() => {
    let cancelled = false;
    const toastId = `team-invitation:${invitation.invitationId}`;
    const title = t`You've been invited to join ${invitation.workspaceName}`;
    const description = invitation.invitedByEmail
      ? t`Invited by ${invitation.invitedByEmail}`
      : undefined;

    sonnerToast(title, {
      id: toastId,
      duration: Infinity,
      description,
      action: {
        label: t`View`,
        onClick: () => {
          openNew({ type: "settings", state: { tab: "team" } });
          sonnerToast.dismiss(toastId);
        },
      },
    });

    const notify = async () => {
      if (
        notificationsDisabled ||
        !(await isAppWindowInactive()) ||
        cancelled
      ) {
        return;
      }
      const result = await notificationCommands.showNotification({
        key: toastId,
        title,
        message: description ?? "",
        timeout: { secs: 15, nanos: 0 },
        source: null,
        start_time: null,
        participants: null,
        event_details: null,
        action_label: t`View`,
        action_variant: null,
        options: null,
        footer: null,
        icon: null,
      });
      if (result.status === "error") {
        console.error(
          "[team] failed to show invitation notification",
          result.error,
        );
      }
    };
    void notify().catch((error) => {
      console.error("[team] failed to show invitation notification", error);
    });

    return () => {
      cancelled = true;
      sonnerToast.dismiss(toastId);
    };
  });

  return null;
}
