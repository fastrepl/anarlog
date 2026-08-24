import { Trans } from "@lingui/react/macro";
import { CircleNotch, X } from "@phosphor-icons/react";
import { useState } from "react";
import type { ReactNode } from "react";

import { Button } from "@anlg/ui/components/ui/button";
import { cn } from "@anlg/utils";

import { isInviteEmail } from "./invitation-management";

import { useHumans } from "~/contacts/queries";
import { ContactFacehash } from "~/contacts/shared";
import { useSessionParticipants } from "~/session/queries";

export function useShareInvite({
  sessionId,
  ownerEmail,
  invitedEmails,
}: {
  sessionId: string;
  ownerEmail: string;
  invitedEmails: string[];
}) {
  const participants = useSessionParticipants(sessionId);
  const [query, setQuery] = useState("");
  const [added, setAdded] = useState<{ email: string; name: string }[]>([]);
  const [dismissed, setDismissed] = useState<string[]>([]);

  const excluded = new Set(
    [ownerEmail, ...invitedEmails]
      .map((email) => email.trim().toLowerCase())
      .filter(Boolean),
  );
  // Recipients are derived on every render so participants that arrive from the
  // live query after the panel opened still appear, while a person the user
  // removed stays removed.
  const taken = new Set(dismissed);
  const recipients: { email: string; name: string }[] = [];
  for (const candidate of [
    ...participants
      .filter((participant) => participant.source !== "excluded")
      .map((participant) => ({
        email: participant.email.trim(),
        name: participant.name.trim(),
      })),
    ...added,
  ]) {
    const key = candidate.email.toLowerCase();
    if (
      !isInviteEmail(candidate.email) ||
      excluded.has(key) ||
      taken.has(key)
    ) {
      continue;
    }
    taken.add(key);
    recipients.push(candidate);
  }

  const typed = query.trim();
  const isSelectable = (email: string) => {
    const key = email.trim().toLowerCase();
    return (
      isInviteEmail(email) &&
      !excluded.has(key) &&
      !recipients.some((recipient) => recipient.email.toLowerCase() === key)
    );
  };
  const add = (recipient: { email: string; name: string }) => {
    const key = recipient.email.trim().toLowerCase();
    setDismissed((current) => current.filter((email) => email !== key));
    setAdded((current) =>
      current.some((entry) => entry.email.toLowerCase() === key)
        ? current
        : [
            ...current,
            { email: recipient.email.trim(), name: recipient.name.trim() },
          ],
    );
    setQuery("");
  };
  const emails = isSelectable(typed)
    ? [...recipients.map((recipient) => recipient.email), typed]
    : recipients.map((recipient) => recipient.email);

  return {
    query,
    setQuery,
    recipients,
    emails,
    canSubmit: emails.length > 0 && (!typed || isInviteEmail(typed)),
    isSelectable,
    add,
    commitQuery: () => {
      if (!isInviteEmail(typed)) return;
      if (isSelectable(typed)) add({ email: typed, name: "" });
      else setQuery("");
    },
    remove: (email: string) => {
      const key = email.trim().toLowerCase();
      setAdded((current) =>
        current.filter((entry) => entry.email.toLowerCase() !== key),
      );
      setDismissed((current) =>
        current.includes(key) ? current : [...current, key],
      );
    },
    restore: (email: string) => {
      const key = email.trim().toLowerCase();
      setDismissed((current) =>
        current.filter((dismissedEmail) => dismissedEmail !== key),
      );
    },
  };
}

export function ShareInviteForm({
  invite,
  disabled,
  pending,
  onSubmit,
  actionLabel,
  inputLabel = "Invitee email",
  placeholder = "Email or name",
}: {
  invite: ReturnType<typeof useShareInvite>;
  disabled: boolean;
  pending: boolean;
  onSubmit: (emails: string[]) => void;
  actionLabel?: ReactNode;
  inputLabel?: string;
  placeholder?: string;
}) {
  const humans = useHumans();
  const normalized = invite.query.trim().toLowerCase();
  const suggestions =
    !normalized || isInviteEmail(normalized)
      ? []
      : humans
          .filter(
            (human) =>
              human.email &&
              invite.isSelectable(human.email) &&
              `${human.name}\n${human.email}`
                .toLowerCase()
                .includes(normalized),
          )
          .slice(0, 4);

  return (
    <>
      <form
        className="flex items-start gap-2"
        onSubmit={(event) => {
          event.preventDefault();
          event.stopPropagation();
          const emails = invite.emails;
          invite.commitQuery();
          onSubmit(emails);
        }}
      >
        <div
          className={cn([
            "border-input focus-within:ring-ring flex h-8 min-w-0 flex-1 items-center rounded-full border bg-transparent px-3 shadow-xs focus-within:ring-1",
            disabled && "opacity-50",
          ])}
        >
          <input
            type="text"
            aria-label={inputLabel}
            autoComplete="email"
            value={invite.query}
            disabled={disabled}
            onChange={(event) => invite.setQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && isInviteEmail(invite.query)) {
                event.preventDefault();
                invite.commitQuery();
              }
            }}
            placeholder={placeholder}
            className="placeholder:text-muted-foreground min-w-0 flex-1 bg-transparent text-xs outline-hidden disabled:cursor-not-allowed"
          />
        </div>
        <Button
          type="submit"
          size="sm"
          disabled={disabled || !invite.canSubmit}
          className="h-8 shrink-0 rounded-full px-3"
        >
          {pending ? (
            <CircleNotch className="size-3.5 animate-spin" aria-hidden="true" />
          ) : null}
          {actionLabel ?? <Trans>Invite</Trans>}
          {invite.emails.length ? (
            <span aria-hidden="true">({invite.emails.length})</span>
          ) : null}
        </Button>
      </form>

      {suggestions.length ? (
        <div className="mt-1 space-y-0.5 rounded-lg border p-1">
          {suggestions.map((contact) => (
            <button
              key={contact.id}
              type="button"
              className="hover:bg-accent flex w-full items-center gap-2 rounded-md px-2 py-1 text-left"
              onClick={() =>
                invite.add({ email: contact.email, name: contact.name })
              }
            >
              <ContactFacehash name={contact.name || contact.email} size={22} />
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
      ) : null}
    </>
  );
}

export function ShareInviteSuggestions({
  invite,
  disabled,
}: {
  invite: ReturnType<typeof useShareInvite>;
  disabled: boolean;
}) {
  if (!invite.recipients.length) return null;

  return (
    <div className="mt-2">
      <div className="px-1.5">
        <h4 className="text-muted-foreground text-[10px] font-medium">
          <Trans>Suggested attendees</Trans>
        </h4>
        <p className="text-muted-foreground mt-0.5 text-[10px] leading-4">
          <Trans>
            Not invited yet. Nothing is sent until you click Invite.
          </Trans>
        </p>
      </div>
      <div className="mt-1 space-y-0.5">
        <ShareInviteRecipientRows invite={invite} disabled={disabled} />
      </div>
    </div>
  );
}

export function ShareInviteRecipientRows({
  invite,
  disabled,
}: {
  invite: ReturnType<typeof useShareInvite>;
  disabled: boolean;
}) {
  return invite.recipients.map((recipient) => {
    const label = recipient.name || recipient.email;
    return (
      <div
        key={recipient.email.toLowerCase()}
        className="hover:bg-accent/50 flex min-h-9 items-center gap-2 rounded-lg px-1.5 py-1"
      >
        <ContactFacehash name={label} size={24} />
        <div className="min-w-0 flex-1">
          <p className="truncate text-xs font-medium">{label}</p>
          {recipient.name ? (
            <p className="text-muted-foreground truncate text-[10px]">
              {recipient.email}
            </p>
          ) : null}
        </div>
        <button
          type="button"
          aria-label={`Remove ${label}`}
          disabled={disabled}
          onClick={() => invite.remove(recipient.email)}
          className="text-muted-foreground hover:bg-accent hover:text-foreground flex size-7 shrink-0 items-center justify-center rounded-md disabled:cursor-not-allowed"
        >
          <X className="size-3.5" aria-hidden="true" />
        </button>
      </div>
    );
  });
}
