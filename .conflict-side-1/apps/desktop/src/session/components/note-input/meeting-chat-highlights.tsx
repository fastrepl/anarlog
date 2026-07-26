import { commands as openerCommands } from "@hypr/plugin-opener2";

import {
  formatMeetingPlatform,
  type MeetingChatRecord,
  useMeetingChatRecords,
} from "~/stt/meeting-chat-records";

export function MeetingChatHighlights({ sessionId }: { sessionId: string }) {
  const records = useMeetingChatRecords(sessionId);
  if (records.length === 0) {
    return null;
  }

  return (
    <section
      aria-label="Meeting chat"
      data-meeting-chat-highlights
      className="border-border/70 bg-muted/30 mx-auto mt-4 mb-6 w-full max-w-3xl rounded-xl border px-3 py-2.5"
      onClick={(event) => event.stopPropagation()}
    >
      <h2 className="text-muted-foreground mb-2 text-xs font-medium">
        Meeting chat
      </h2>
      <div className="flex flex-col gap-2">
        {records.map((record) => (
          <MeetingChatRow key={record.id} record={record} />
        ))}
      </div>
    </section>
  );
}

function MeetingChatRow({ record }: { record: MeetingChatRecord }) {
  const platform = formatMeetingPlatform(record.platform);
  const direction =
    record.direction === "outgoing"
      ? "sent"
      : record.direction === "incoming"
        ? "received"
        : null;
  const metadata = [record.timestamp, record.sender, direction]
    .filter((value): value is string => Boolean(value))
    .join(" · ");

  return (
    <div className="text-foreground text-sm leading-5">
      <div className="text-muted-foreground text-xs">
        {platform}
        {metadata ? ` · ${metadata}` : null}
      </div>
      <p className="whitespace-pre-wrap">
        <MeetingChatText record={record} />
      </p>
    </div>
  );
}

function MeetingChatText({ record }: { record: MeetingChatRecord }) {
  const segments = splitMeetingChatText(record.text, record.links);

  return segments.map((segment, index) =>
    segment.link ? (
      <a
        key={`${segment.text}-${index}`}
        href={segment.link}
        className="text-primary underline underline-offset-2"
        onClick={(event) => {
          event.preventDefault();
          void openerCommands.openUrl(segment.link!, null);
        }}
      >
        {segment.text}
      </a>
    ) : (
      segment.text
    ),
  );
}

function splitMeetingChatText(text: string, links: string[]) {
  const uniqueLinks = [...new Set(links)].filter(
    (link) => /^https?:\/\//.test(link) && text.includes(link),
  );
  const segments: Array<{ text: string; link?: string }> = [];
  let cursor = 0;

  while (cursor < text.length) {
    const nextLink = uniqueLinks
      .map((link) => ({ link, index: text.indexOf(link, cursor) }))
      .filter(({ index }) => index >= 0)
      .sort((left, right) => left.index - right.index)[0];

    if (!nextLink) {
      segments.push({ text: text.slice(cursor) });
      break;
    }
    if (nextLink.index > cursor) {
      segments.push({ text: text.slice(cursor, nextLink.index) });
    }
    segments.push({ text: nextLink.link, link: nextLink.link });
    cursor = nextLink.index + nextLink.link.length;
  }

  return segments.length > 0 ? segments : [{ text }];
}
