import { Trans } from "@lingui/react/macro";
import { CircleNotch, X } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useState } from "react";
import type { ReactNode } from "react";

import { colors, radii, shadows } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";

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
        {...stylex.props(styles.form)}
        onSubmit={(event) => {
          event.preventDefault();
          event.stopPropagation();
          const emails = invite.emails;
          invite.commitQuery();
          onSubmit(emails);
        }}
      >
        <div
          {...stylex.props(
            styles.inputShell,
            disabled && styles.inputShellDisabled,
          )}
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
            {...stylex.props(styles.input)}
          />
        </div>
        <Button
          type="submit"
          size="sm"
          disabled={disabled || !invite.canSubmit}
          sx={styles.submitButton}
        >
          {pending ? (
            <CircleNotch {...stylex.props(styles.spinner)} aria-hidden="true" />
          ) : null}
          {actionLabel ?? <Trans>Invite</Trans>}
          {invite.emails.length ? (
            <span aria-hidden="true">({invite.emails.length})</span>
          ) : null}
        </Button>
      </form>

      {suggestions.length ? (
        <div {...stylex.props(styles.suggestions)}>
          {suggestions.map((contact) => (
            <button
              key={contact.id}
              type="button"
              {...stylex.props(styles.suggestionButton)}
              onClick={() =>
                invite.add({ email: contact.email, name: contact.name })
              }
            >
              <ContactFacehash name={contact.name || contact.email} size={22} />
              <span {...stylex.props(styles.recipientContent)}>
                <span {...stylex.props(styles.recipientName)}>
                  {contact.name || contact.email}
                </span>
                {contact.name ? (
                  <span {...stylex.props(styles.recipientEmail)}>
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
    <div {...stylex.props(styles.suggestionSection)}>
      <h4 {...stylex.props(styles.sectionHeading)}>
        <Trans>Suggested attendees</Trans>
      </h4>
      <div {...stylex.props(styles.recipientList)}>
        <ShareInviteRecipientRows
          invite={invite}
          disabled={disabled}
          status={<Trans>Not invited</Trans>}
        />
      </div>
    </div>
  );
}

export function ShareInviteRecipientRows({
  invite,
  disabled,
  status,
}: {
  invite: ReturnType<typeof useShareInvite>;
  disabled: boolean;
  status?: ReactNode;
}) {
  return invite.recipients.map((recipient) => {
    const label = recipient.name || recipient.email;
    return (
      <div
        key={recipient.email.toLowerCase()}
        {...stylex.props(styles.recipientRow)}
      >
        <ContactFacehash name={label} size={24} />
        <div {...stylex.props(styles.recipientContent)}>
          <p {...stylex.props(styles.recipientName)}>{label}</p>
          {recipient.name ? (
            <p {...stylex.props(styles.recipientEmail)}>{recipient.email}</p>
          ) : null}
        </div>
        {status ? <span {...stylex.props(styles.status)}>{status}</span> : null}
        <button
          type="button"
          aria-label={`Remove ${label}`}
          disabled={disabled}
          onClick={() => invite.remove(recipient.email)}
          {...stylex.props(styles.removeButton)}
        >
          <X {...stylex.props(styles.icon)} aria-hidden="true" />
        </button>
      </div>
    );
  });
}

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  form: {
    alignItems: "flex-start",
    display: "flex",
    gap: "0.5rem",
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  input: {
    "::placeholder": {
      color: colors.mutedForeground,
    },
    backgroundColor: "transparent",
    cursor: {
      default: "text",
      ":disabled": "not-allowed",
    },
    flex: "1",
    fontSize: "0.75rem",
    minWidth: 0,
    outline: "none",
  },
  inputShell: {
    alignItems: "center",
    backgroundColor: "transparent",
    borderColor: colors.input,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: {
      default: shadows.sm,
      ":focus-within": `0 0 0 1px ${colors.ring}`,
    },
    display: "flex",
    flex: "1",
    height: "2rem",
    minWidth: 0,
    paddingInline: "0.75rem",
  },
  inputShellDisabled: {
    opacity: 0.5,
  },
  recipientContent: {
    flex: "1",
    minWidth: 0,
  },
  recipientEmail: {
    color: colors.mutedForeground,
    display: "block",
    fontSize: "0.625rem",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  recipientList: {
    display: "flex",
    flexDirection: "column",
    gap: "0.125rem",
    marginTop: "0.25rem",
  },
  recipientName: {
    display: "block",
    fontSize: "0.75rem",
    fontWeight: 500,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  recipientRow: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": `color-mix(in srgb, ${colors.accent} 50%, transparent)`,
    },
    borderRadius: radii.lg,
    display: "flex",
    gap: "0.5rem",
    minHeight: "2.25rem",
    paddingBlock: "0.25rem",
    paddingInline: "0.375rem",
  },
  removeButton: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: radii.md,
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    cursor: {
      default: "pointer",
      ":disabled": "not-allowed",
    },
    display: "flex",
    flexShrink: 0,
    height: "1.75rem",
    justifyContent: "center",
    width: "1.75rem",
  },
  sectionHeading: {
    color: colors.mutedForeground,
    fontSize: "0.625rem",
    fontWeight: 500,
    paddingInline: "0.375rem",
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    height: "1rem",
    width: "1rem",
  },
  status: {
    color: colors.mutedForeground,
    flexShrink: 0,
    fontSize: "0.6875rem",
  },
  submitButton: {
    borderRadius: radii.full,
    flexShrink: 0,
    height: "2rem",
    paddingInline: "0.75rem",
  },
  suggestionButton: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: radii.md,
    display: "flex",
    gap: "0.5rem",
    paddingBlock: "0.25rem",
    paddingInline: "0.5rem",
    textAlign: "left",
    width: "100%",
  },
  suggestionSection: {
    marginTop: "0.5rem",
  },
  suggestions: {
    borderColor: colors.border,
    borderRadius: radii.lg,
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    flexDirection: "column",
    gap: "0.125rem",
    marginTop: "0.25rem",
    padding: "0.25rem",
  },
});

export { styles as shareInviteStyles };
