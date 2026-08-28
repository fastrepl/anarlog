import { Trans } from "@lingui/react/macro";
import { CircleNotch, Copy } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useState } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  PopoverContent,
} from "@anlg/ui/components/ui/popover";

import {
  defaultGeneralAccessTarget,
  normalizeDefaultMeetingShareAccess,
} from "./default-access";
import {
  EmailRecapForm,
  ShareRecapOverflowMenu,
  SlackRecapForm,
  type ShareRecapMode,
} from "./delivery-panel";
import {
  GeneralAccessSelector,
  type GeneralAccessTarget,
} from "./general-access";
import {
  ShareInviteForm,
  ShareInviteSuggestions,
  useShareInvite,
} from "./invite-recipients";
import type { AvailableShareWorkspace } from "./source";
import { useWorkspaceShareScopes } from "./workspace-policy";

import { useAuth } from "~/auth";
import { ContactFacehash } from "~/contacts/shared";
import { useConfigValue } from "~/shared/config";

export type DraftShareAction =
  | { type: "invite"; emails: string[] }
  | { type: "email"; emails: string[] }
  | { type: "slack"; channel: { id: string; name: string } }
  | { type: "copy-link" }
  | { type: "scope"; target: GeneralAccessTarget };

export function SessionShareDraftContent({
  sessionId,
  disabled,
  pendingAction,
  workspaces,
  onAction,
}: {
  sessionId: string;
  disabled: boolean;
  pendingAction: DraftShareAction | null;
  workspaces: AvailableShareWorkspace[];
  onAction: (action: DraftShareAction) => void;
}) {
  const auth = useAuth();
  const [recapMode, setRecapMode] = useState<ShareRecapMode>("invite");
  const ownerEmail = auth.session?.user.email ?? "";
  const ownerMetadata = auth.session?.user.user_metadata;
  const ownerName =
    typeof ownerMetadata?.full_name === "string" && ownerMetadata.full_name
      ? ownerMetadata.full_name
      : typeof ownerMetadata?.name === "string" && ownerMetadata.name
        ? ownerMetadata.name
        : ownerEmail || "You";
  const invite = useShareInvite({ sessionId, ownerEmail, invitedEmails: [] });
  const actionPending = pendingAction !== null;
  const allowedScopes = useWorkspaceShareScopes(workspaces);
  const defaultAccess = normalizeDefaultMeetingShareAccess(
    useConfigValue("default_meeting_share_access"),
  );
  const generalAccessValue =
    pendingAction?.type === "scope"
      ? pendingAction.target
      : defaultGeneralAccessTarget(defaultAccess, workspaces);

  return (
    <PopoverContent
      variant="app"
      align="end"
      sideOffset={8}
      aria-labelledby="session-share-heading"
      aria-describedby="session-share-description"
      sx={styles.popover}
    >
      <AppFloatingPanel sx={styles.panel}>
        <h2 id="session-share-heading" {...stylex.props(styles.visuallyHidden)}>
          <Trans>Share</Trans>
        </h2>
        <p
          id="session-share-description"
          {...stylex.props(styles.visuallyHidden)}
        >
          <Trans>Invite people to this note.</Trans>
        </p>

        <div {...stylex.props(styles.scrollArea)}>
          <div {...stylex.props(styles.content)}>
            {recapMode === "invite" ? (
              <section aria-labelledby="invite-people-heading">
                <h3
                  id="invite-people-heading"
                  {...stylex.props(styles.visuallyHidden)}
                >
                  <Trans>Invite people</Trans>
                </h3>
                <ShareInviteForm
                  invite={invite}
                  disabled={disabled || actionPending}
                  pending={pendingAction?.type === "invite"}
                  onSubmit={(emails) => onAction({ type: "invite", emails })}
                />

                <ShareInviteSuggestions
                  invite={invite}
                  disabled={disabled || actionPending}
                />

                <div {...stylex.props(styles.accessSection)}>
                  <h4 {...stylex.props(styles.accessHeading)}>
                    <Trans>People with access</Trans>
                  </h4>
                  <div {...stylex.props(styles.ownerRow)}>
                    <ContactFacehash name={ownerName} size={24} />
                    <div {...stylex.props(styles.ownerContent)}>
                      <p {...stylex.props(styles.ownerName)}>
                        {ownerName}{" "}
                        <span {...stylex.props(styles.muted)}>(You)</span>
                      </p>
                      {ownerEmail ? (
                        <p {...stylex.props(styles.ownerEmail)}>{ownerEmail}</p>
                      ) : null}
                    </div>
                    <span {...stylex.props(styles.accessLabel)}>
                      <Trans>Full access</Trans>
                    </span>
                  </div>
                </div>
              </section>
            ) : recapMode === "email" ? (
              <EmailRecapForm
                sessionId={sessionId}
                ownerEmail={ownerEmail}
                disabled={disabled || actionPending}
                pending={pendingAction?.type === "email"}
                onBack={() => setRecapMode("invite")}
                onSubmit={(emails) => onAction({ type: "email", emails })}
              />
            ) : (
              <SlackRecapForm
                disabled={disabled || actionPending}
                pending={pendingAction?.type === "slack"}
                onBack={() => setRecapMode("invite")}
                onSubmit={(channel) => onAction({ type: "slack", channel })}
              />
            )}
          </div>
        </div>

        <footer {...stylex.props(styles.footer)}>
          <GeneralAccessSelector
            value={generalAccessValue}
            workspaces={workspaces}
            disabled={disabled}
            canExpand={!disabled}
            pending={pendingAction?.type === "scope"}
            allowedScopes={allowedScopes}
            onValueChange={(target) => {
              if (target !== "restricted") {
                onAction({ type: "scope", target });
              }
            }}
          />
          <ShareRecapOverflowMenu onValueChange={setRecapMode} />
          <Button
            type="button"
            size="sm"
            variant="outline"
            disabled={disabled || actionPending}
            onClick={() => onAction({ type: "copy-link" })}
            sx={styles.copyButton}
          >
            {pendingAction?.type === "copy-link" ? (
              <CircleNotch
                {...stylex.props(styles.icon, styles.spinner)}
                aria-hidden="true"
              />
            ) : (
              <Copy {...stylex.props(styles.icon)} aria-hidden="true" />
            )}
            <Trans>Copy link</Trans>
          </Button>
        </footer>
      </AppFloatingPanel>
    </PopoverContent>
  );
}

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  accessHeading: {
    color: colors.mutedForeground,
    fontSize: "0.625rem",
    fontWeight: 500,
    marginBottom: "0.25rem",
    paddingInline: "0.375rem",
  },
  accessLabel: {
    color: colors.mutedForeground,
    flexShrink: 0,
    fontSize: "0.6875rem",
  },
  accessSection: {
    borderTopColor: `color-mix(in srgb, ${colors.border} 60%, transparent)`,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    marginTop: "0.5rem",
    paddingTop: "0.5rem",
  },
  content: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
  },
  copyButton: {
    borderRadius: radii.md,
    flexShrink: 0,
    fontSize: "0.75rem",
    height: "1.75rem",
    paddingInline: "0.625rem",
  },
  footer: {
    alignItems: "center",
    borderTopColor: `color-mix(in srgb, ${colors.border} 60%, transparent)`,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    display: "flex",
    gap: "0.25rem",
    paddingBlock: "0.5rem",
    paddingInline: "0.75rem",
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  muted: {
    color: colors.mutedForeground,
  },
  ownerContent: {
    flex: "1",
    minWidth: 0,
  },
  ownerEmail: {
    color: colors.mutedForeground,
    fontSize: "0.625rem",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  ownerName: {
    fontSize: "0.75rem",
    fontWeight: 500,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  ownerRow: {
    alignItems: "center",
    borderRadius: radii.lg,
    display: "flex",
    gap: "0.5rem",
    minHeight: "2.25rem",
    paddingBlock: "0.25rem",
    paddingInline: "0.375rem",
  },
  panel: {
    display: "flex",
    flexDirection: "column",
    maxHeight: "min(530px, calc(100vh - 74px))",
    overflow: "hidden",
  },
  popover: {
    maxWidth: "calc(100vw - 16px)",
    overflow: "hidden",
    width: "440px",
  },
  scrollArea: {
    flex: "1",
    minHeight: 0,
    overflowY: "auto",
    overscrollBehavior: "contain",
    paddingBlock: "0.5rem",
    paddingInline: "0.75rem",
    scrollbarColor:
      "hsl(var(--muted-foreground, 25 5% 45%) / 0.18) transparent",
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
  },
  visuallyHidden: {
    borderWidth: 0,
    clipPath: "inset(50%)",
    height: "1px",
    margin: "-1px",
    overflow: "hidden",
    padding: 0,
    position: "absolute",
    whiteSpace: "nowrap",
    width: "1px",
  },
});
