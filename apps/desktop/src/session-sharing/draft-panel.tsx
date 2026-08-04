import { Trans } from "@lingui/react/macro";
import { CircleNotch, Copy } from "@phosphor-icons/react";
import { useForm } from "@tanstack/react-form";

import { Button } from "@anlg/ui/components/ui/button";
import { Input } from "@anlg/ui/components/ui/input";
import {
  AppFloatingPanel,
  PopoverContent,
} from "@anlg/ui/components/ui/popover";

import {
  GeneralAccessSelector,
  type GeneralAccessTarget,
} from "./general-access";
import { isInviteEmail } from "./invitation-management";
import type { AvailableShareWorkspace } from "./source";

import { useAuth } from "~/auth";
import { useHumans } from "~/contacts/queries";
import { ContactFacehash } from "~/contacts/shared";

export type DraftShareAction =
  | { type: "invite"; email: string }
  | { type: "copy-link" }
  | { type: "scope"; target: GeneralAccessTarget };

export function SessionShareDraftContent({
  disabled,
  pendingAction,
  workspaces,
  onAction,
}: {
  disabled: boolean;
  pendingAction: DraftShareAction | null;
  workspaces: AvailableShareWorkspace[];
  onAction: (action: DraftShareAction) => void;
}) {
  const auth = useAuth();
  const humans = useHumans();
  const form = useForm({
    defaultValues: { email: "" },
    onSubmit: ({ value }) => {
      onAction({ type: "invite", email: value.email.trim() });
    },
  });
  const ownerEmail = auth.session?.user.email ?? "";
  const ownerMetadata = auth.session?.user.user_metadata;
  const ownerName =
    typeof ownerMetadata?.full_name === "string" && ownerMetadata.full_name
      ? ownerMetadata.full_name
      : typeof ownerMetadata?.name === "string" && ownerMetadata.name
        ? ownerMetadata.name
        : ownerEmail || "You";
  const suggestedContacts = (query: string) => {
    const normalized = query.trim().toLowerCase();
    if (!normalized || isInviteEmail(normalized)) return [];
    return humans
      .filter(
        (human) =>
          human.email &&
          human.email.toLowerCase() !== ownerEmail.toLowerCase() &&
          `${human.name}\n${human.email}`.toLowerCase().includes(normalized),
      )
      .slice(0, 4);
  };
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
      className="grid max-h-[min(540px,calc(100vh-64px))] min-h-[340px] w-[440px] max-w-[calc(100vw-16px)] overflow-hidden"
    >
      <AppFloatingPanel className="flex min-h-0 flex-col overflow-hidden">
        <header className="border-border/60 border-b px-3 py-2 text-left">
          <h2
            id="session-share-heading"
            className="text-sm leading-5 font-semibold tracking-normal"
          >
            <Trans>Share</Trans>
          </h2>
          <p id="session-share-description" className="sr-only">
            <Trans>Invite people to this note.</Trans>
          </p>
        </header>

        <div className="scrollbar-soft min-h-0 flex-1 overflow-y-auto overscroll-contain px-3 py-2">
          <div className="space-y-2">
            <section aria-labelledby="invite-people-heading">
              <h3 id="invite-people-heading" className="sr-only">
                <Trans>People with access</Trans>
              </h3>
              <form
                className="flex items-center gap-2"
                onSubmit={(event) => {
                  event.preventDefault();
                  event.stopPropagation();
                  void form.handleSubmit();
                }}
              >
                <form.Field name="email">
                  {(field) => (
                    <Input
                      type="text"
                      aria-label="Invitee email"
                      autoComplete="email"
                      required
                      value={field.state.value}
                      disabled={disabled || actionPending}
                      onBlur={field.handleBlur}
                      onChange={(event) =>
                        field.handleChange(event.target.value)
                      }
                      placeholder="Email or name"
                      className="h-8 min-w-0 flex-1 rounded-md text-xs"
                    />
                  )}
                </form.Field>
                <form.Subscribe selector={(state) => state.values.email}>
                  {(email) => (
                    <Button
                      type="submit"
                      size="sm"
                      disabled={
                        disabled || actionPending || !isInviteEmail(email)
                      }
                      className="h-8 shrink-0 rounded-md px-3"
                    >
                      {pendingAction?.type === "invite" ? (
                        <CircleNotch
                          className="size-3.5 animate-spin"
                          aria-hidden="true"
                        />
                      ) : null}
                      <Trans>Invite</Trans>
                    </Button>
                  )}
                </form.Subscribe>
              </form>

              <form.Subscribe selector={(state) => state.values.email}>
                {(query) => {
                  const suggestions = suggestedContacts(query);
                  return suggestions.length ? (
                    <div className="mt-1 space-y-0.5 rounded-lg border p-1">
                      {suggestions.map((contact) => (
                        <button
                          key={contact.id}
                          type="button"
                          className="hover:bg-accent flex w-full items-center gap-2 rounded-md px-2 py-1 text-left"
                          onClick={() =>
                            form.setFieldValue("email", contact.email)
                          }
                        >
                          <ContactFacehash
                            name={contact.name || contact.email}
                            size={22}
                          />
                          <span className="min-w-0 flex-1">
                            <span className="block truncate text-xs font-medium">
                              {contact.name || contact.email}
                            </span>
                            {contact.name ? (
                              <span className="text-muted-foreground block truncate text-[10px]">
                                {contact.email}
                              </span>
                            ) : null}
                          </span>
                        </button>
                      ))}
                    </div>
                  ) : null;
                }}
              </form.Subscribe>

              <div className="mt-2 flex min-h-9 items-center gap-2 rounded-lg px-1.5 py-1">
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
