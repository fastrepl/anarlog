import { Trans, useLingui } from "@lingui/react/macro";
import {
  CircleNotch,
  EnvelopeSimple,
  LockSimple,
  SlackLogo,
  UserPlus,
} from "@phosphor-icons/react";
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";

import { Button } from "@anlg/ui/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@anlg/ui/components/ui/select";
import { cn } from "@anlg/utils";

import { listSlackChannels } from "./delivery-client";
import {
  ShareInviteForm,
  ShareInviteRecipientRows,
  useShareInvite,
} from "./invite-recipients";

import { useAuth } from "~/auth";
import { useConnections } from "~/auth/useConnections";
import { env } from "~/env";
import { useOpenIntegrationUrl } from "~/shared/integration";

export type ShareRecapMode = "invite" | "email" | "slack";

export function ShareRecapModeSelector({
  value,
  onValueChange,
}: {
  value: ShareRecapMode;
  onValueChange: (value: ShareRecapMode) => void;
}) {
  return (
    <div className="bg-muted/60 grid grid-cols-3 gap-0.5 rounded-lg p-0.5">
      {(
        [
          ["invite", UserPlus],
          ["email", EnvelopeSimple],
          ["slack", SlackLogo],
        ] as const
      ).map(([mode, Icon]) => (
        <button
          key={mode}
          type="button"
          aria-pressed={value === mode}
          onClick={() => onValueChange(mode)}
          className={cn([
            "flex h-7 items-center justify-center gap-1.5 rounded-md text-xs transition-colors",
            value === mode
              ? "bg-background text-foreground shadow-xs"
              : "text-muted-foreground hover:text-foreground",
          ])}
        >
          <Icon className="size-3.5" aria-hidden="true" />
          {mode === "invite" ? (
            <Trans>People</Trans>
          ) : mode === "email" ? (
            <Trans>Email</Trans>
          ) : (
            <Trans>Slack</Trans>
          )}
        </button>
      ))}
    </div>
  );
}

export function EmailRecapForm({
  sessionId,
  ownerEmail,
  disabled,
  pending,
  onSubmit,
}: {
  sessionId: string;
  ownerEmail: string;
  disabled: boolean;
  pending: boolean;
  onSubmit: (emails: string[]) => void;
}) {
  const { t } = useLingui();
  const recipients = useShareInvite({
    sessionId,
    ownerEmail,
    invitedEmails: [],
  });

  return (
    <section aria-labelledby="email-recap-heading" className="space-y-2">
      <div>
        <h3 id="email-recap-heading" className="text-xs font-medium">
          <Trans>Email meeting notes</Trans>
        </h3>
        <p className="text-muted-foreground mt-0.5 text-[11px] leading-4">
          <Trans>
            Send the summary in the email. Replies go directly to you.
          </Trans>
        </p>
      </div>
      <ShareInviteForm
        invite={recipients}
        disabled={disabled || pending}
        pending={pending}
        inputLabel={t`Recipient email`}
        placeholder={t`Email or name`}
        actionLabel={<Trans>Send</Trans>}
        onSubmit={onSubmit}
      />
      <ShareInviteRecipientRows
        invite={recipients}
        disabled={disabled || pending}
      />
    </section>
  );
}

export function SlackRecapForm({
  disabled,
  pending,
  onSubmit,
}: {
  disabled: boolean;
  pending: boolean;
  onSubmit: (channel: { id: string; name: string }) => void;
}) {
  const { t } = useLingui();
  const auth = useAuth();
  const connections = useConnections(true);
  const { openIntegration, openingAction } = useOpenIntegrationUrl();
  const slackConnection = connections.data?.find(
    (connection) => connection.integration_id === "slack",
  );
  const connected = Boolean(
    slackConnection && slackConnection.status !== "reconnect_required",
  );
  const channels = useQuery({
    queryKey: ["slack-channels", auth.session?.user.id],
    enabled: connected && Boolean(auth.session?.access_token),
    queryFn: ({ signal }) =>
      listSlackChannels({
        apiBaseUrl: env.VITE_API_URL,
        accessToken: auth.session?.access_token ?? "",
        signal,
      }),
  });
  const [channelId, setChannelId] = useState("");
  const selectedChannel = channels.data?.find(
    (channel) => channel.id === channelId,
  );

  if (!slackConnection || slackConnection.status === "reconnect_required") {
    const reconnect = slackConnection?.status === "reconnect_required";
    return (
      <section aria-labelledby="slack-recap-heading" className="space-y-2">
        <div>
          <h3 id="slack-recap-heading" className="text-xs font-medium">
            <Trans>Send to Slack</Trans>
          </h3>
          <p className="text-muted-foreground mt-0.5 text-[11px] leading-4">
            <Trans>Connect Slack to choose a channel for this recap.</Trans>
          </p>
        </div>
        <Button
          type="button"
          size="sm"
          variant="outline"
          disabled={disabled || openingAction !== null}
          onClick={() =>
            openIntegration({
              nangoIntegrationId: "slack",
              connectionId: slackConnection?.connection_id,
              action: reconnect ? "reconnect" : "connect",
            })
          }
          className="h-8"
        >
          {openingAction ? (
            <CircleNotch className="size-3.5 animate-spin" aria-hidden="true" />
          ) : (
            <SlackLogo className="size-3.5" aria-hidden="true" />
          )}
          {reconnect ? (
            <Trans>Reconnect Slack</Trans>
          ) : (
            <Trans>Connect Slack</Trans>
          )}
        </Button>
      </section>
    );
  }

  return (
    <section aria-labelledby="slack-recap-heading" className="space-y-2">
      <div>
        <h3 id="slack-recap-heading" className="text-xs font-medium">
          <Trans>Send to Slack</Trans>
        </h3>
        <p className="text-muted-foreground mt-0.5 text-[11px] leading-4">
          <Trans>Post the meeting summary to a channel you can access.</Trans>
        </p>
      </div>
      <div className="flex gap-2">
        <Select
          value={channelId}
          onValueChange={setChannelId}
          disabled={disabled || pending || channels.isLoading}
        >
          <SelectTrigger className="h-8 min-w-0 flex-1 text-xs">
            <SelectValue
              placeholder={
                channels.isLoading ? t`Loading channels…` : t`Choose a channel`
              }
            />
          </SelectTrigger>
          <SelectContent>
            {channels.data?.map((channel) => (
              <SelectItem key={channel.id} value={channel.id}>
                <span className="flex items-center gap-1.5">
                  {channel.isPrivate ? (
                    <LockSimple className="size-3" aria-hidden="true" />
                  ) : (
                    <span aria-hidden="true">#</span>
                  )}
                  {channel.name}
                </span>
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <Button
          type="button"
          size="sm"
          disabled={disabled || pending || !selectedChannel}
          onClick={() => {
            if (selectedChannel) onSubmit(selectedChannel);
          }}
          className="h-8 shrink-0"
        >
          {pending ? (
            <CircleNotch className="size-3.5 animate-spin" aria-hidden="true" />
          ) : null}
          <Trans>Send</Trans>
        </Button>
      </div>
      {channels.isError ? (
        <p className="text-destructive text-[11px]">
          <Trans>
            Could not load Slack channels. Reconnect Slack and try again.
          </Trans>
        </p>
      ) : null}
    </section>
  );
}
