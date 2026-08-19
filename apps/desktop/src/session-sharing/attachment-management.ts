import { t } from "@lingui/core/macro";
import { useMutation } from "@tanstack/react-query";

import { sonnerToast } from "@anlg/ui/components/ui/toast";

import {
  prepareSessionShareAttachment,
  type SessionShareAttachment,
} from "./attachments";
import { ShareManagementError } from "./client";
import {
  ShareOperationAbortedError,
  type SharePanelIdentity,
} from "./management";
import {
  type PublishLatestSessionShare,
  type RequireActiveShareContext,
  type RunShareOperation,
} from "./management-operation";

import { env } from "~/env";
import type { SharedNoteAttachment } from "~/shared-notes/cache";

type AttachmentMutation = {
  attachment: SessionShareAttachment;
  included: boolean;
};

export function useSessionAttachmentManagement({
  identity,
  managementAvailable,
  canExpand,
  sharedAttachments,
  sharedAttachmentIds,
  runOperation,
  publishLatest,
  requireActiveContext,
  onChanged,
}: {
  identity: SharePanelIdentity;
  managementAvailable: boolean;
  canExpand: boolean;
  sharedAttachments: SharedNoteAttachment[];
  sharedAttachmentIds: Map<string, string>;
  runOperation: RunShareOperation;
  publishLatest: PublishLatestSessionShare;
  requireActiveContext: RequireActiveShareContext;
  onChanged: () => Promise<unknown>;
}) {
  return useMutation({
    mutationFn: (input: AttachmentMutation) =>
      runOperation(async (signal) => {
        if (!managementAvailable) throw new ShareManagementError();
        const { attachment } = input;
        const currentId = sharedAttachmentIds.get(attachment.id);
        if (!canExpand) throw new ShareManagementError();
        let requested = [...sharedAttachments];
        if (!input.included) {
          requested = requested.filter((item) => item.id !== currentId);
          return publishLatest(signal, requested);
        }

        const context = requireActiveContext(signal);
        const prepared = await prepareSessionShareAttachment({
          apiBaseUrl: env.VITE_API_URL,
          supabaseUrl: env.VITE_SUPABASE_URL ?? "",
          session: context.session,
          shareId: identity.shareId,
          attachment,
          signal,
        });
        requested = [
          ...requested.filter((item) => item.id !== currentId),
          prepared,
        ];
        return publishLatest(
          signal,
          requested,
          new Map([[attachment.id, prepared.id]]),
        );
      }),
    onSuccess: () => {
      sonnerToast.success(t`Attachment settings updated.`);
    },
    onError: (error) => {
      if (error instanceof ShareOperationAbortedError) return;
      sonnerToast.error(t`Could not update attachment sharing.`);
    },
    onSettled: onChanged,
  });
}
