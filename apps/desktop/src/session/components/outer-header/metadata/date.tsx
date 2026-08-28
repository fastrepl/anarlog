import { useLingui } from "@lingui/react/macro";
import { Check, Pencil, X } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useForm } from "@tanstack/react-form";
import { useState } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";
import { Input } from "@anlg/ui/components/ui/input";
import { sonnerToast } from "@anlg/ui/components/ui/toast";
import { format, safeFormat, safeParseDate } from "@anlg/utils";

import { useSession, useUpdateSession } from "~/session/queries";

export function DateEditor({ sessionId }: { sessionId: string }) {
  const { t } = useLingui();
  const [isEditing, setIsEditing] = useState(false);
  // Shown between closing the editor and the live query re-emitting, so the
  // read-only label never flashes the pre-save date. It masks the live value
  // until that value catches up (or the write fails), not until the write
  // resolves — the live query can lag the commit.
  const [pendingCreatedAt, setPendingCreatedAt] = useState<string | null>(null);
  const createdAt = useSession(sessionId)?.created_at;
  const effectiveCreatedAt =
    pendingCreatedAt !== null && createdAt !== pendingCreatedAt
      ? pendingCreatedAt
      : createdAt;
  const noteDate = safeFormat(
    effectiveCreatedAt ?? new Date(),
    "MMM d, yyyy h:mm a",
    t`Unknown date`,
  );

  if (!isEditing) {
    return (
      <div {...stylex.props(styles.readonlyRow)}>
        <div {...stylex.props(styles.dateContainer)}>
          <div {...stylex.props(styles.date)}>{noteDate}</div>
        </div>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          sx={styles.editButton}
          onClick={() => setIsEditing(true)}
          aria-label={t`Edit date`}
        >
          <Pencil size={16} />
        </Button>
      </div>
    );
  }

  return (
    <EditableDateForm
      key={`${createdAt ?? ""}`}
      sessionId={sessionId}
      createdAt={createdAt}
      onCancel={() => setIsEditing(false)}
      onSaved={(nextCreatedAt, commit) => {
        setIsEditing(false);
        setPendingCreatedAt(nextCreatedAt);
        void commit.catch((error) => {
          console.error("[metadata] failed to update session date", error);
          sonnerToast.error(t`Could not update the note date.`);
          setPendingCreatedAt(null);
        });
      }}
    />
  );
}

function EditableDateForm({
  sessionId,
  createdAt,
  onCancel,
  onSaved,
}: {
  sessionId: string;
  createdAt: unknown;
  onCancel?: () => void;
  onSaved?: (nextCreatedAt: string, commit: Promise<unknown>) => void;
}) {
  const { t } = useLingui();
  const updateSession = useUpdateSession(sessionId);

  const form = useForm({
    defaultValues: {
      createdAt: toDatetimeLocalValue(createdAt),
    },
    validators: {
      onChange: ({ value }) => {
        if (!value.createdAt.trim()) {
          return {
            fields: {
              createdAt: t`Date and time are required`,
            },
          };
        }

        if (!toIsoString(value.createdAt)) {
          return {
            fields: {
              createdAt: t`Enter a valid date and time`,
            },
          };
        }

        return undefined;
      },
    },
    onSubmit: ({ value }) => {
      const nextCreatedAt = toIsoString(value.createdAt);
      if (!nextCreatedAt) {
        return;
      }

      onSaved?.(
        nextCreatedAt,
        Promise.resolve(updateSession({ created_at: nextCreatedAt })),
      );
    },
  });

  return (
    <div {...stylex.props(styles.form)}>
      <form.Field name="createdAt">
        {(field) => (
          <div {...stylex.props(styles.editRow)}>
            <Input
              autoFocus
              type="datetime-local"
              sx={styles.input}
              value={field.state.value}
              onChange={(e) => field.handleChange(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  void form.handleSubmit();
                }

                if (e.key === "Escape") {
                  e.preventDefault();
                  onCancel?.();
                }
              }}
            />

            {onCancel && (
              <Button
                type="button"
                variant="ghost"
                size="icon"
                sx={[styles.iconButton, styles.cancelButton]}
                onClick={onCancel}
                aria-label={t`Cancel date edit`}
              >
                <X size={16} />
              </Button>
            )}

            <form.Subscribe selector={(state) => [state.canSubmit]}>
              {([canSubmit]) => (
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  sx={[styles.iconButton, styles.saveButton]}
                  onClick={() => void form.handleSubmit()}
                  disabled={!canSubmit}
                  aria-label={t`Save date`}
                >
                  <Check size={16} />
                </Button>
              )}
            </form.Subscribe>
          </div>
        )}
      </form.Field>

      <form.Field name="createdAt">
        {(field) =>
          field.state.meta.errors[0] ? (
            <div {...stylex.props(styles.error)}>
              {field.state.meta.errors[0]}
            </div>
          ) : null
        }
      </form.Field>
    </div>
  );
}

function toDatetimeLocalValue(value: unknown): string {
  const date = safeParseDate(value);
  if (!date) {
    return "";
  }

  return format(date, "yyyy-MM-dd'T'HH:mm");
}

function toIsoString(value: string): string | null {
  const parsed = safeParseDate(value);
  return parsed?.toISOString() ?? null;
}

const styles = stylex.create({
  cancelButton: {
    backgroundColor: {
      default: "transparent",
      ":hover": "#fef2f2",
    },
    color: {
      default: colors.mutedForeground,
      ":hover": "#dc2626",
    },
  },
  date: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
  },
  dateContainer: {
    flex: "1",
    minWidth: 0,
  },
  editButton: {
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: radii.full,
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    height: "1.75rem",
    width: "1.75rem",
  },
  editRow: {
    alignItems: "center",
    display: "flex",
    gap: 0,
    height: "1.75rem",
  },
  error: {
    color: "#dc2626",
    fontSize: "0.75rem",
  },
  form: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
  },
  iconButton: {
    borderRadius: radii.full,
    flexShrink: 0,
    height: "1.75rem",
    width: "1.75rem",
  },
  input: {
    borderWidth: 0,
    boxShadow: {
      default: "none",
      ":focus-visible": "none",
    },
    flex: "1",
    height: "1.75rem",
    padding: 0,
  },
  readonlyRow: {
    alignItems: "center",
    display: "flex",
    gap: "0.75rem",
    height: "1.75rem",
    justifyContent: "space-between",
  },
  saveButton: {
    backgroundColor: {
      default: "transparent",
      ":hover": "#f0fdf4",
    },
    color: {
      default: colors.mutedForeground,
      ":hover": "#16a34a",
    },
  },
});

export { styles as dateEditorStyles };
