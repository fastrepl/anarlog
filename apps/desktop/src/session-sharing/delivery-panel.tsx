import { Icon } from "@iconify-icon/react";
import { Trans, useLingui } from "@lingui/react/macro";
import {
  CaretLeft,
  CircleNotch,
  DotsThree,
  EnvelopeSimple,
  LockSimple,
} from "@phosphor-icons/react";
import { useQuery } from "@tanstack/react-query";
import { useState, type ReactNode } from "react";

import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@anlg/ui/components/ui/select";

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

function SlackBrandIcon({ size }: { size: number }) {
  return (
    <Icon
      icon="logos:slack-icon"
      width={size}
      height={size}
      aria-hidden="true"
    />
  );
}

export function ShareRecapOverflowMenu({
  onValueChange,
}: {
  onValueChange: (value: Exclude<ShareRecapMode, "invite">) => void;
}) {
  const { t } = useLingui();

  return (
    <DropdownMenu modal={false}>
      <DropdownMenuTrigger asChild>
        <Button
          type="button"
          size="icon"
          variant="ghost"
          aria-label={t`More options`}
          className="text-muted-foreground hover:text-foreground"
        >
          <DotsThree className="size-4" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent variant="app" align="end" className="w-44">
        <AppFloatingPanel className="overflow-hidden p-1">
          <DropdownMenuItem
            onSelect={() => onValueChange("email")}
            className="cursor-pointer"
          >
            <EnvelopeSimple aria-hidden="true" />
            <Trans>Email</Trans>
          </DropdownMenuItem>
          <DropdownMenuItem
            onSelect={() => onValueChange("slack")}
            className="cursor-pointer"
          >
            <span className="flex size-4 items-center justify-center">
              <SlackBrandIcon size={16} />
            </span>
            <Trans>Slack</Trans>
          </DropdownMenuItem>
        </AppFloatingPanel>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function ShareRecapFormHeading({
  id,
  onBack,
  children,
}: {
  id: string;
  onBack?: () => void;
  children: ReactNode;
}) {
  const { t } = useLingui();

  return (
    <div className="flex items-center gap-1">
      {onBack ? (
        <button
          type="button"
          onClick={onBack}
          aria-label={t`Back`}
          className="text-muted-foreground hover:text-foreground flex size-6 shrink-0 items-center justify-center rounded-md"
        >
          <CaretLeft className="size-4" aria-hidden="true" />
        </button>
      ) : null}
      <h3 id={id} className="text-xs font-medium">
        {children}
      </h3>
    </div>
  );
}

export function EmailRecapForm({
  sessionId,
  ownerEmail,
  disabled,
  pending,
  onBack,
  onSubmit,
}: {
  sessionId: string;
  ownerEmail: string;
  disabled: boolean;
  pending: boolean;
  onBack?: () => void;
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
        <ShareRecapFormHeading id="email-recap-heading" onBack={onBack}>
          <Trans>Email meeting notes</Trans>
        </ShareRecapFormHeading>
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
  onBack,
  onSubmit,
}: {
  disabled: boolean;
  pending: boolean;
  onBack?: () => void;
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
          <ShareRecapFormHeading id="slack-recap-heading" onBack={onBack}>
            <Trans>Send to Slack</Trans>
          </ShareRecapFormHeading>
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
            <CircleNotch className="size-4 animate-spin" aria-hidden="true" />
          ) : (
            <SlackBrandIcon size={16} />
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
        <ShareRecapFormHeading id="slack-recap-heading" onBack={onBack}>
          <Trans>Send to Slack</Trans>
        </ShareRecapFormHeading>
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
            <CircleNotch className="size-4 animate-spin" aria-hidden="true" />
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
