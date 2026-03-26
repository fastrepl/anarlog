import { safeFormat } from "@hypr/utils";

import * as main from "~/store/tinybase/store/main";

export function DateDisplay({ sessionId }: { sessionId: string }) {
  const createdAt = main.UI.useCell(
    "sessions",
    sessionId,
    "created_at",
    main.STORE_ID,
  );
  const { startedAt, endedAt } = useSessionRecordingTimes(sessionId);

  const displayDate = !startedAt
    ? safeFormat(createdAt ?? new Date(), "MMM d, yyyy", "Unknown date")
    : !endedAt
      ? safeFormat(startedAt, "MMM d, yyyy h:mm a", "Unknown date")
      : `${safeFormat(startedAt, "MMM d, yyyy h:mm a")} - ${safeFormat(endedAt, "MMM d, yyyy h:mm a")}`;

  return (
    <div className="flex flex-col gap-1">
      <div className="text-sm font-medium text-neutral-700">{displayDate}</div>
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
