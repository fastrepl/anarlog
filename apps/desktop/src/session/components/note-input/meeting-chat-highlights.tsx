import { t } from "@lingui/core/macro";
import * as stylex from "@stylexjs/stylex";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { commands as openerCommands } from "@anlg/plugin-opener2";

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
      aria-label={t`Meeting chat`}
      data-meeting-chat-highlights
      {...stylex.props(styles.root)}
      onClick={(event) => event.stopPropagation()}
    >
      <h2 {...stylex.props(styles.heading)}>{t`Meeting chat`}</h2>
      <div {...stylex.props(styles.rows)}>
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
      ? t`sent`
      : record.direction === "incoming"
        ? t`received`
        : null;
  const metadata = [record.timestamp, record.sender, direction]
    .filter((value): value is string => Boolean(value))
    .join(" · ");

  return (
    <div {...stylex.props(styles.row)}>
      <div {...stylex.props(styles.metadata)}>
        {platform}
        {metadata ? ` · ${metadata}` : null}
      </div>
      <p {...stylex.props(styles.text)}>
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
        {...stylex.props(styles.link)}
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

const styles = stylex.create({
  heading: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    fontWeight: 500,
    marginBottom: "0.5rem",
  },
  link: {
    color: colors.primary,
    textDecorationLine: "underline",
    textUnderlineOffset: "2px",
  },
  metadata: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
  },
  root: {
    backgroundColor: `color-mix(in oklab, ${colors.muted} 30%, transparent)`,
    borderColor: `color-mix(in oklab, ${colors.border} 70%, transparent)`,
    borderRadius: radii.xl,
    borderStyle: "solid",
    borderWidth: "1px",
    marginBlockEnd: "1.5rem",
    marginBlockStart: "1rem",
    marginInline: "auto",
    maxWidth: "48rem",
    paddingBlock: "0.625rem",
    paddingInline: "0.75rem",
    width: "100%",
  },
  row: {
    color: colors.foreground,
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
  },
  rows: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
  },
  text: {
    whiteSpace: "pre-wrap",
  },
});
