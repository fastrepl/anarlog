import { Trans } from "@lingui/react/macro";
import { CircleNotch, Copy } from "@phosphor-icons/react";

import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  PopoverContent,
} from "@anlg/ui/components/ui/popover";

import {
  GeneralAccessSelector,
  type GeneralAccessTarget,
} from "./general-access";
import {
  ShareInviteForm,
  ShareInviteRecipientRows,
  useShareInvite,
} from "./invite-recipients";
import type { AvailableShareWorkspace } from "./source";

import { useAuth } from "~/auth";
import { ContactFacehash } from "~/contacts/shared";

export type DraftShareAction =
  | { type: "invite"; emails: string[] }
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
  const generalAccessValue =
    pendingAction?.type === "scope" ? pendingAction.target : "restricted";

  return (
    <PopoverContent
      variant="app"
      align="end"
      sideOffset={8}
      aria-labelledby="session-share-heading"
      aria-describedby="session-share-description"
      className="w-[440px] max-w-[calc(100vw-16px)] overflow-hidden"
    >
      <AppFloatingPanel className="flex max-h-[min(530px,calc(100vh-74px))] flex-col overflow-hidden">
        <h2 id="session-share-heading" className="sr-only">
          <Trans>Share</Trans>
        </h2>
        <p id="session-share-description" className="sr-only">
          <Trans>Invite people to this note.</Trans>
        </p>

        <div className="scrollbar-soft min-h-0 flex-1 overflow-y-auto overscroll-contain px-3 py-2">
          <div className="space-y-2">
            <section aria-labelledby="invite-people-heading">
              <h3 id="invite-people-heading" className="sr-only">
                <Trans>People with access</Trans>
              </h3>
              <ShareInviteForm
                invite={invite}
                disabled={disabled || actionPending}
                pending={pendingAction?.type === "invite"}
                onSubmit={(emails) => onAction({ type: "invite", emails })}
              />

              <div className="mt-2 space-y-0.5">
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

                <ShareInviteRecipientRows
                  invite={invite}
                  disabled={disabled || actionPending}
                />
              </div>
            </section>

            <section
              aria-labelledby="general-access-heading"
              className="border-border/60 border-t pt-2"
            >
              <h3
                id="general-access-heading"
                className="text-muted-foreground mb-1 text-[10px] font-medium"
              >
                <Trans>General access</Trans>
              </h3>
              <GeneralAccessSelector
                value={generalAccessValue}
                workspaces={workspaces}
                disabled={disabled}
                canExpand={!disabled}
                pending={pendingAction?.type === "scope"}
                onValueChange={(target) => {
                  if (target !== "restricted") {
                    onAction({ type: "scope", target });
                  }
                }}
              />
            </section>
          </div>
        </div>

        <footer className="border-border/60 flex justify-end border-t px-3 py-2">
          <Button
            type="button"
            size="sm"
            variant="outline"
            disabled={disabled || actionPending}
            onClick={() => onAction({ type: "copy-link" })}
            className="h-7 rounded-md px-2.5 text-xs"
          >
            {pendingAction?.type === "copy-link" ? (
              <CircleNotch
                className="size-3.5 animate-spin"
                aria-hidden="true"
              />
            ) : (
              <Copy className="size-3.5" aria-hidden="true" />
            )}
            <Trans>Copy link</Trans>
          </Button>
        </footer>
      </AppFloatingPanel>
    </PopoverContent>
  );
}
