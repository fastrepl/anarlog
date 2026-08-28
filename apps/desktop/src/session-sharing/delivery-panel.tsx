import { Icon } from "@iconify-icon/react";
import { Trans, useLingui } from "@lingui/react/macro";
import {
  CaretLeft,
  CircleNotch,
  DotsThree,
  EnvelopeSimple,
  LockSimple,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useQuery } from "@tanstack/react-query";
import { useState, type ReactNode } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
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
          sx={styles.mutedButton}
        >
          <DotsThree {...stylex.props(styles.icon)} />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent variant="app" align="end" sx={styles.menu}>
        <AppFloatingPanel sx={styles.menuPanel}>
          <DropdownMenuItem
            onSelect={() => onValueChange("email")}
            sx={styles.menuItem}
          >
            <EnvelopeSimple aria-hidden="true" />
            <Trans>Email</Trans>
          </DropdownMenuItem>
          <DropdownMenuItem
            onSelect={() => onValueChange("slack")}
            sx={styles.menuItem}
          >
            <span {...stylex.props(styles.iconContainer)}>
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
    <div {...stylex.props(styles.heading)}>
      {onBack ? (
        <button
          type="button"
          onClick={onBack}
          aria-label={t`Back`}
          {...stylex.props(styles.backButton)}
        >
          <CaretLeft {...stylex.props(styles.icon)} aria-hidden="true" />
        </button>
      ) : null}
      <h3 id={id} {...stylex.props(styles.headingText)}>
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
    <section
      aria-labelledby="email-recap-heading"
      {...stylex.props(styles.form)}
    >
      <div>
        <ShareRecapFormHeading id="email-recap-heading" onBack={onBack}>
          <Trans>Email meeting notes</Trans>
        </ShareRecapFormHeading>
        <p {...stylex.props(styles.description)}>
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
      <section
        aria-labelledby="slack-recap-heading"
        {...stylex.props(styles.form)}
      >
        <div>
          <ShareRecapFormHeading id="slack-recap-heading" onBack={onBack}>
            <Trans>Send to Slack</Trans>
          </ShareRecapFormHeading>
          <p {...stylex.props(styles.description)}>
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
          sx={styles.tallButton}
        >
          {openingAction ? (
            <CircleNotch {...stylex.props(styles.spinner)} aria-hidden="true" />
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
    <section
      aria-labelledby="slack-recap-heading"
      {...stylex.props(styles.form)}
    >
      <div>
        <ShareRecapFormHeading id="slack-recap-heading" onBack={onBack}>
          <Trans>Send to Slack</Trans>
        </ShareRecapFormHeading>
        <p {...stylex.props(styles.description)}>
          <Trans>Post the meeting summary to a channel you can access.</Trans>
        </p>
      </div>
      <div {...stylex.props(styles.channelActions)}>
        <Select
          value={channelId}
          onValueChange={setChannelId}
          disabled={disabled || pending || channels.isLoading}
        >
          <SelectTrigger sx={styles.channelTrigger}>
            <SelectValue
              placeholder={
                channels.isLoading ? t`Loading channels…` : t`Choose a channel`
              }
            />
          </SelectTrigger>
          <SelectContent>
            {channels.data?.map((channel) => (
              <SelectItem key={channel.id} value={channel.id}>
                <span {...stylex.props(styles.channelOption)}>
                  {channel.isPrivate ? (
                    <LockSimple
                      {...stylex.props(styles.privateIcon)}
                      aria-hidden="true"
                    />
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
          sx={styles.sendButton}
        >
          {pending ? (
            <CircleNotch {...stylex.props(styles.spinner)} aria-hidden="true" />
          ) : null}
          <Trans>Send</Trans>
        </Button>
      </div>
      {channels.isError ? (
        <p {...stylex.props(styles.error)}>
          <Trans>
            Could not load Slack channels. Reconnect Slack and try again.
          </Trans>
        </p>
      ) : null}
    </section>
  );
}

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  backButton: {
    alignItems: "center",
    borderRadius: radii.md,
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    display: "flex",
    flexShrink: 0,
    height: "1.5rem",
    justifyContent: "center",
    width: "1.5rem",
  },
  channelActions: {
    display: "flex",
    gap: "0.5rem",
  },
  channelOption: {
    alignItems: "center",
    display: "flex",
    gap: "0.375rem",
  },
  channelTrigger: {
    flex: "1",
    fontSize: "0.75rem",
    height: "2rem",
    minWidth: 0,
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.6875rem",
    lineHeight: "1rem",
    marginTop: "0.125rem",
  },
  error: {
    color: colors.destructive,
    fontSize: "0.6875rem",
  },
  form: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
  },
  heading: {
    alignItems: "center",
    display: "flex",
    gap: "0.25rem",
  },
  headingText: {
    fontSize: "0.75rem",
    fontWeight: 500,
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  iconContainer: {
    alignItems: "center",
    display: "flex",
    height: "1rem",
    justifyContent: "center",
    width: "1rem",
  },
  menu: {
    width: "11rem",
  },
  menuItem: {
    cursor: "pointer",
  },
  menuPanel: {
    overflow: "hidden",
    padding: "0.25rem",
  },
  mutedButton: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
  },
  privateIcon: {
    height: "0.75rem",
    width: "0.75rem",
  },
  sendButton: {
    flexShrink: 0,
    height: "2rem",
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    height: "1rem",
    width: "1rem",
  },
  tallButton: {
    height: "2rem",
  },
});
