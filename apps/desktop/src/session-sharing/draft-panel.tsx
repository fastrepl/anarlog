import { Trans } from "@lingui/react/macro";
import { CircleNotch, Copy } from "@phosphor-icons/react";
import { useState, type ReactNode } from "react";

import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  PopoverContent,
} from "@anlg/ui/components/ui/popover";
import { cn } from "@anlg/utils";

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
  gate = null,
}: {
  sessionId: string;
  disabled: boolean;
  pendingAction: DraftShareAction | null;
  workspaces: AvailableShareWorkspace[];
  onAction: (action: DraftShareAction) => void;
  gate?: {
    title: ReactNode;
    description: ReactNode;
    action: ReactNode;
  } | null;
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

  const labelledBy = gate
    ? "session-share-sign-in-heading"
    : "session-share-heading";
  const describedBy = gate
    ? "session-share-sign-in-description"
    : "session-share-description";

  return (
    <PopoverContent
      variant="app"
      align="end"
      sideOffset={8}
      aria-labelledby={labelledBy}
      aria-describedby={describedBy}
      className="w-[440px] max-w-[calc(100vw-16px)] overflow-hidden"
    >
      <AppFloatingPanel className="relative flex max-h-[min(530px,calc(100vh-74px))] flex-col overflow-hidden">
        <div
          inert={gate ? true : undefined}
          className={cn([
            "flex min-h-0 flex-1 flex-col overflow-hidden",
            gate && "pointer-events-none blur-[3px] select-none",
          ])}
        >
          <h2 id="session-share-heading" className="sr-only">
            <Trans>Share</Trans>
          </h2>
          <p id="session-share-description" className="sr-only">
            <Trans>Invite people to this note.</Trans>
          </p>

          <div className="scrollbar-soft min-h-0 flex-1 overflow-y-auto overscroll-contain px-3 py-2">
            <div className="space-y-2">
              {recapMode === "invite" ? (
                <section aria-labelledby="invite-people-heading">
                  <h3 id="invite-people-heading" className="sr-only">
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

                  <div className="border-border/60 mt-2 border-t pt-2">
                    <h4 className="text-muted-foreground mb-1 px-1.5 text-[10px] font-medium">
                      <Trans>People with access</Trans>
                    </h4>
                    <div className="flex min-h-9 items-center gap-2 rounded-lg px-1.5 py-1">
                      <ContactFacehash name={ownerName} size={24} />
                      <div className="min-w-0 flex-1">
                        <p className="truncate text-xs font-medium">
                          {ownerName}{" "}
                          <span className="text-muted-foreground">(You)</span>
                        </p>
                        {ownerEmail ? (
                          <p className="text-muted-foreground truncate text-[10px]">
                            {ownerEmail}
                          </p>
                        ) : null}
                      </div>
                      <span className="text-muted-foreground shrink-0 text-[11px]">
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

          <footer className="border-border/60 flex items-center gap-1 border-t px-3 py-2">
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
              className="h-7 shrink-0 rounded-md px-2.5 text-xs"
            >
              {pendingAction?.type === "copy-link" ? (
                <CircleNotch
                  className="size-4 animate-spin"
                  aria-hidden="true"
                />
              ) : (
                <Copy className="size-4" aria-hidden="true" />
              )}
              <Trans>Copy link</Trans>
            </Button>
          </footer>
        </div>
        {gate ? (
          <div className="bg-background/70 absolute inset-0 z-10 flex items-center justify-center px-6">
            <div className="max-w-[220px] text-center">
              <h2
                id="session-share-sign-in-heading"
                className="text-sm font-semibold"
              >
                {gate.title}
              </h2>
              <p
                id="session-share-sign-in-description"
                className="text-muted-foreground mt-1 text-xs leading-5"
              >
                {gate.description}
              </p>
              <div className="mt-4">{gate.action}</div>
            </div>
          </div>
        ) : null}
      </AppFloatingPanel>
    </PopoverContent>
  );
}
