import { useMutation } from "@tanstack/react-query";

import { json2md } from "@anlg/editor/markdown";
import type { JSONContent } from "@anlg/editor/note";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import type { ShareManagementContext } from "./client";
import {
  buildSlackRecap,
  sendSessionShareRecapEmail,
  sendSlackRecap,
  ShareDeliveryError,
} from "./delivery-client";
import { getSessionShareSenderName } from "./invitation-management";
import { ShareOperationAbortedError } from "./management";
import type {
  PublishLatestSessionShare,
  RequireActiveShareContext,
  RunShareOperation,
} from "./management-operation";

import { trackAnalyticsEvent } from "~/analytics";
import { env } from "~/env";

export async function deliverSessionShareRecapEmail({
  context,
  shareId,
  recipients,
  noteTitle,
  body,
  signal,
}: {
  context: ShareManagementContext;
  shareId: string;
  recipients: string[];
  noteTitle: string;
  body: JSONContent;
  signal: AbortSignal;
}) {
  const noteBody = json2md(body).trim();
  if (!noteBody) throw new ShareDeliveryError();
  await sendSessionShareRecapEmail({
    apiBaseUrl: env.VITE_API_URL,
    session: context.session,
    shareId,
    recipients,
    senderName: getSessionShareSenderName(context.session.user),
    noteTitle,
    noteBody,
    deliveryId: crypto.randomUUID(),
    signal,
  });
}

export async function deliverSessionShareRecapToSlack({
  context,
  channel,
  noteTitle,
  body,
  signal,
}: {
  context: ShareManagementContext;
  channel: { id: string; name: string };
  noteTitle: string;
  body: JSONContent;
  signal: AbortSignal;
}) {
  const noteBody = json2md(body).trim();
  if (!noteBody) throw new ShareDeliveryError();
  await sendSlackRecap({
    apiBaseUrl: env.VITE_API_URL,
    accessToken: context.session.access_token,
    channel: channel.id,
    text: buildSlackRecap({
      senderName: getSessionShareSenderName(context.session.user),
      noteTitle,
      noteBody,
    }),
    signal,
  });
}

export function useSessionRecapDelivery({
  shareId,
  canDeliver,
  runOperation,
  publishLatest,
  requireActiveContext,
  onActivated,
}: {
  shareId: string;
  canDeliver: boolean;
  runOperation: RunShareOperation;
  publishLatest: PublishLatestSessionShare;
  requireActiveContext: RequireActiveShareContext;
  onActivated: () => Promise<unknown>;
}) {
  const emailMutation = useMutation({
    mutationFn: (recipients: string[]) =>
      runOperation(async (signal) => {
        if (!canDeliver) throw new ShareDeliveryError();
        const published = await publishLatest(signal);
        const context = requireActiveContext(signal);
        await deliverSessionShareRecapEmail({
          context,
          shareId,
          recipients,
          noteTitle: published.title,
          body: published.body,
          signal,
        });
        await onActivated();
        return recipients.length;
      }),
    onSuccess: (recipientCount) => {
      trackAnalyticsEvent("share_recap_sent", {
        delivery_method: "email",
        recipient_count: recipientCount,
      });
      sonnerToast.success(
        recipientCount > 1 ? "Meeting notes sent." : "Meeting note sent.",
      );
    },
    onError: (error) => {
      if (error instanceof ShareOperationAbortedError) return;
      sonnerToast.error("Could not email the meeting notes.");
    },
  });
  const slackMutation = useMutation({
    mutationFn: (channel: { id: string; name: string }) =>
      runOperation(async (signal) => {
        if (!canDeliver) throw new ShareDeliveryError();
        const published = await publishLatest(signal);
        const context = requireActiveContext(signal);
        await deliverSessionShareRecapToSlack({
          context,
          channel,
          noteTitle: published.title,
          body: published.body,
          signal,
        });
        await onActivated();
        return channel;
      }),
    onSuccess: (channel) => {
      trackAnalyticsEvent("share_recap_sent", {
        delivery_method: "slack",
      });
      sonnerToast.success(`Meeting notes sent to #${channel.name}.`);
    },
    onError: (error) => {
      if (error instanceof ShareOperationAbortedError) return;
      sonnerToast.error("Could not send the meeting notes to Slack.");
    },
  });

  return { emailMutation, slackMutation };
}
