import { useForm } from "@tanstack/react-form";
import { CalendarIcon, CheckIcon, XIcon } from "lucide-react";
import { useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import { Input } from "@hypr/ui/components/ui/input";
import { format, safeFormat, safeParseDate } from "@hypr/utils";

import * as main from "~/store/tinybase/store/main";

export function DateEditor({ sessionId }: { sessionId: string }) {
  const [isEditing, setIsEditing] = useState(false);
  const createdAt = main.UI.useCell(
    "sessions",
    sessionId,
    "created_at",
    main.STORE_ID,
  );
  const noteDate = safeFormat(
    createdAt ?? new Date(),
    "MMM d, yyyy h:mm a",
    "Unknown date",
  );

  if (!isEditing) {
    return (
      <div className="flex h-9 items-center justify-between gap-3">
        <div className="min-w-0 flex-1">
          <div className="text-sm text-neutral-700">{noteDate}</div>
        </div>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="size-7 rounded-md text-neutral-500 hover:text-neutral-900"
          onClick={() => setIsEditing(true)}
          aria-label="Edit date"
        >
          <CalendarIcon size={16} />
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
      onSaved={() => setIsEditing(false)}
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
  onSaved?: () => void;
}) {
  const handleChangeCreatedAt = main.UI.useSetCellCallback(
    "sessions",
    sessionId,
    "created_at",
    (value: string) => value,
    [],
    main.STORE_ID,
  );

  const form = useForm({
    defaultValues: {
      createdAt: toDatetimeLocalValue(createdAt),
    },
    validators: {
      onChange: ({ value }) => {
        if (!value.createdAt.trim()) {
          return {
            fields: {
              createdAt: "Date and time are required",
            },
          };
        }

        if (!toIsoString(value.createdAt)) {
          return {
            fields: {
              createdAt: "Enter a valid date and time",
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

      handleChangeCreatedAt(nextCreatedAt);
      onSaved?.();
    },
  });

  return (
    <div className="flex flex-col gap-2">
      <form.Field name="createdAt">
        {(field) => (
          <div className="flex h-9 items-center gap-0">
            <Input
              autoFocus
              type="datetime-local"
              className="flex-1 border-0 px-0 shadow-none focus-visible:ring-0"
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
                className="size-7 shrink-0 rounded-md text-neutral-400 hover:bg-red-50 hover:text-red-600"
                onClick={onCancel}
                aria-label="Cancel date edit"
              >
                <XIcon size={16} />
              </Button>
            )}

            <form.Subscribe selector={(state) => [state.canSubmit]}>
              {([canSubmit]) => (
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className="size-7 shrink-0 rounded-md text-neutral-400 hover:bg-green-50 hover:text-green-600"
                  onClick={() => void form.handleSubmit()}
                  disabled={!canSubmit}
                  aria-label="Save date"
                >
                  <CheckIcon size={16} />
                </Button>
              )}
            </form.Subscribe>
          </div>
        )}
      </form.Field>

      <form.Field name="createdAt">
        {(field) =>
          field.state.meta.errors[0] ? (
            <div className="text-xs text-red-600">
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
