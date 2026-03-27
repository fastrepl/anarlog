import { useForm } from "@tanstack/react-form";
import { useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import { Input } from "@hypr/ui/components/ui/input";
import { format, safeFormat, safeParseDate } from "@hypr/utils";

import * as main from "~/store/tinybase/store/main";

export function DateDisplay({ sessionId }: { sessionId: string }) {
  const [isEditing, setIsEditing] = useState(false);
  const createdAt = main.UI.useCell(
    "sessions",
    sessionId,
    "created_at",
    main.STORE_ID,
  );
  const { startedAt, endedAt } = useSessionRecordingTimes(sessionId);
  const noteDate = safeFormat(
    createdAt ?? new Date(),
    "MMM d, yyyy h:mm a",
    "Unknown date",
  );
  const recordingTime = formatRecordingTime(startedAt, endedAt);

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <div className="text-base font-medium text-neutral-900">
            {noteDate}
          </div>
          {recordingTime && (
            <div className="mt-1 text-sm text-neutral-500">
              Recording: {recordingTime}
            </div>
          )}
        </div>

        {!isEditing && (
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => setIsEditing(true)}
          >
            Edit
          </Button>
        )}
      </div>

      {isEditing && (
        <EditableDateForm
          key={`${createdAt ?? ""}`}
          sessionId={sessionId}
          createdAt={createdAt}
          onCancel={() => setIsEditing(false)}
        />
      )}
    </div>
  );
}

function EditableDateForm({
  sessionId,
  createdAt,
  onCancel,
}: {
  sessionId: string;
  createdAt: unknown;
  onCancel: () => void;
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
      onCancel();
    },
  });

  return (
    <div className="flex flex-col gap-2 rounded-lg border border-neutral-200 p-3">
      <form.Field name="createdAt">
        {(field) => (
          <div className="flex flex-col gap-2">
            <Input
              autoFocus
              type="datetime-local"
              value={field.state.value}
              onChange={(e) => field.handleChange(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  void form.handleSubmit();
                }

                if (e.key === "Escape") {
                  e.preventDefault();
                  onCancel();
                }
              }}
            />

            {field.state.meta.errors[0] && (
              <div className="text-xs text-red-600">
                {field.state.meta.errors[0]}
              </div>
            )}
          </div>
        )}
      </form.Field>

      <form.Subscribe selector={(state) => [state.canSubmit]}>
        {([canSubmit]) => (
          <div className="flex justify-end gap-2">
            <Button type="button" variant="ghost" size="sm" onClick={onCancel}>
              Cancel
            </Button>
            <Button
              type="button"
              size="sm"
              onClick={() => void form.handleSubmit()}
              disabled={!canSubmit}
            >
              Save
            </Button>
          </div>
        )}
      </form.Subscribe>
    </div>
  );
}

function useSessionRecordingTimes(sessionId: string) {
  const resultTable = main.UI.useResultTable(
    main.QUERIES.sessionRecordingTimes,
    main.STORE_ID,
  );

  const recordingTimes = Object.values(resultTable).find(
    (row) => row.session_id === sessionId,
  );

  return {
    startedAt: recordingTimes?.min_started_at as number | undefined,
    endedAt: recordingTimes?.max_ended_at as number | undefined,
  };
}

function formatRecordingTime(
  startedAt?: number,
  endedAt?: number,
): string | null {
  if (!startedAt) {
    return null;
  }

  if (!endedAt) {
    return safeFormat(startedAt, "MMM d, yyyy h:mm a", "Unknown date");
  }

  return `${safeFormat(startedAt, "MMM d, yyyy h:mm a")} - ${safeFormat(endedAt, "MMM d, yyyy h:mm a")}`;
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
