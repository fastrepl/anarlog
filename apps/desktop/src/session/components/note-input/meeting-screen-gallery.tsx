import { t } from "@lingui/core/macro";
import { useQuery } from "@tanstack/react-query";
import { convertFileSrc } from "@tauri-apps/api/core";

import { commands as fsSyncCommands } from "@anlg/plugin-fs-sync";

import { sessionAttachmentPathsQueryKey } from "~/session/hooks/useAttachmentResolver";
import { formatMeetingPlatform } from "~/stt/meeting-chat-records";
import {
  type MeetingScreenRecord,
  useMeetingScreenRecords,
} from "~/stt/meeting-screen-records";

export function MeetingScreenGallery({ sessionId }: { sessionId: string }) {
  const records = useMeetingScreenRecords(sessionId);
  const attachmentIds = records.map((record) => record.attachmentId).join(",");
  const { data: sources } = useQuery({
    queryKey: [
      ...sessionAttachmentPathsQueryKey(sessionId),
      "meeting-screens",
      attachmentIds,
    ],
    enabled: records.length > 0,
    queryFn: async () => {
      const result = await fsSyncCommands.attachmentList(sessionId);
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return new Map(
        result.data.map((attachment) => [
          attachment.attachmentId,
          convertFileSrc(attachment.path),
        ]),
      );
    },
  });

  if (records.length === 0) {
    return null;
  }

  return (
    <section
      aria-label={t`Shared screens`}
      data-meeting-screen-gallery
      className="border-border/70 bg-muted/30 mx-auto mt-4 mb-6 w-full max-w-3xl rounded-xl border px-3 py-2.5"
      onClick={(event) => event.stopPropagation()}
    >
      <h2 className="text-muted-foreground mb-2 text-xs font-medium">
        {t`Shared screens`}
      </h2>
      <div className="grid grid-cols-2 gap-2">
        {records.map((record) => (
          <MeetingScreenTile
            key={record.id}
            record={record}
            src={sources?.get(record.attachmentId)}
          />
        ))}
      </div>
    </section>
  );
}

function MeetingScreenTile({
  record,
  src,
}: {
  record: MeetingScreenRecord;
  src: string | undefined;
}) {
  const time = new Date(record.capturedAt).toLocaleTimeString([], {
    hour: "numeric",
    minute: "2-digit",
  });
  const label = record.platform
    ? `${formatMeetingPlatform(record.platform as never)} · ${time}`
    : time;

  return (
    <figure className="flex flex-col gap-1">
      <div
        className="bg-muted overflow-hidden rounded-md"
        style={{
          aspectRatio:
            record.width && record.height
              ? `${record.width} / ${record.height}`
              : "16 / 9",
        }}
      >
        {src ? (
          <img
            src={src}
            alt={t`Shared screen at ${time}`}
            className="h-full w-full object-contain"
            draggable={false}
          />
        ) : null}
      </div>
      <figcaption className="text-muted-foreground text-xs">{label}</figcaption>
    </figure>
  );
}
