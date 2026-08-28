import { CalendarBlank, MapPin, VideoCamera } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { forwardRef, useState } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@anlg/ui/components/ui/popover";
import { safeFormat, safeParseDate, TZDate } from "@anlg/utils";

import { DateEditor } from "./date";
import { ParticipantsDisplay } from "./participants";

import { useSessionEvent } from "~/session/hooks/useSessionEvent";
import { useConfigValue } from "~/shared/config";

export function MetadataButton({ sessionId }: { sessionId: string }) {
  const [open, setOpen] = useState(false);
  const sessionEvent = useSessionEvent(sessionId);
  const label = sessionEvent ? "Open event metadata" : "Open note metadata";

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <TriggerInner label={label} open={open} />
      </PopoverTrigger>
      <PopoverContent variant="app" align="end" sx={styles.popover}>
        <AppFloatingPanel sx={styles.panel}>
          <ContentInner sessionId={sessionId} />
        </AppFloatingPanel>
      </PopoverContent>
    </Popover>
  );
}

const TriggerInner = forwardRef<
  HTMLButtonElement,
  { label: string; open?: boolean }
>(({ label, open, ...props }, ref) => {
  return (
    <Button
      ref={ref}
      {...props}
      variant="ghost"
      size="icon"
      type="button"
      data-tauri-drag-region="false"
      aria-label={label}
      title={label}
      sx={[styles.trigger, open && styles.triggerOpen]}
    >
      <CalendarBlank {...stylex.props(styles.icon)} />
    </Button>
  );
});

function ContentInner({ sessionId }: { sessionId: string }) {
  const sessionEvent = useSessionEvent(sessionId);

  const eventDisplayData = sessionEvent
    ? {
        title: sessionEvent.title,
        startedAt: sessionEvent.started_at,
        endedAt: sessionEvent.ended_at,
        location: sessionEvent.location,
        meetingLink: sessionEvent.meeting_link,
        description: sessionEvent.description,
        calendarId: sessionEvent.calendar_id,
      }
    : null;

  return (
    <div {...stylex.props(styles.content)}>
      {!eventDisplayData && <DateEditor sessionId={sessionId} />}
      {eventDisplayData && (
        <EventDisplay event={eventDisplayData}>
          <ParticipantsDisplay sessionId={sessionId} />
        </EventDisplay>
      )}
      {!eventDisplayData && <ParticipantsDisplay sessionId={sessionId} />}
    </div>
  );
}

export function EventDisplay({
  event,
  children,
}: {
  event: {
    title: string | undefined;
    startedAt: string | undefined;
    endedAt: string | undefined;
    location: string | undefined;
    meetingLink: string | undefined;
    description: string | undefined;
    calendarId: string | undefined;
  };
  children?: React.ReactNode;
}) {
  const tz = useConfigValue("timezone") || undefined;

  const handleJoinMeeting = () => {
    if (event.meetingLink) {
      void openerCommands.openUrl(event.meetingLink, null);
    }
  };

  const toTz = (date: Date): Date => (tz ? new TZDate(date, tz) : date);

  const formatEventDateTime = () => {
    if (!event.startedAt) {
      return "";
    }

    const rawStart = safeParseDate(event.startedAt);
    const rawEnd = event.endedAt ? safeParseDate(event.endedAt) : null;

    if (!rawStart) {
      return "";
    }

    const startDate = toTz(rawStart);
    const endDate = rawEnd ? toTz(rawEnd) : null;

    const startStr = safeFormat(startDate, "MMM d, yyyy h:mm a");
    if (!endDate) {
      return startStr;
    }

    const sameDay = startDate.toDateString() === endDate.toDateString();
    const endStr = sameDay
      ? safeFormat(endDate, "h:mm a")
      : safeFormat(endDate, "MMM d, yyyy h:mm a");

    return `${startStr} to ${endStr}`;
  };

  const getMeetingLinkDomain = () => {
    if (!event.meetingLink) {
      return null;
    }
    try {
      const url = new URL(event.meetingLink);
      return url.hostname.replace("www.", "");
    } catch {
      return null;
    }
  };

  const meetingDomain = getMeetingLinkDomain();

  const isLocationURL = (location: string) => {
    try {
      new URL(location);
      return true;
    } catch {
      return false;
    }
  };

  const shouldShowLocation = event.location && !isLocationURL(event.location);

  return (
    <div {...stylex.props(styles.event)}>
      <div {...stylex.props(styles.title)}>
        {event.title || "Untitled Event"}
      </div>

      <div {...stylex.props(styles.divider)} />

      {shouldShowLocation && (
        <>
          <div {...stylex.props(styles.eventRow)}>
            <MapPin size={16} {...stylex.props(styles.rowIcon)} />
            <span>{event.location}</span>
          </div>
        </>
      )}

      {event.meetingLink && (
        <>
          <div {...stylex.props(styles.meetingRow)}>
            <div {...stylex.props(styles.eventRow, styles.meetingDetails)}>
              <VideoCamera size={16} {...stylex.props(styles.rowIcon)} />
              <span {...stylex.props(styles.truncate)}>
                {meetingDomain || "Meeting link"}
              </span>
            </div>
            <Button
              size="sm"
              variant="default"
              sx={styles.joinButton}
              onClick={handleJoinMeeting}
            >
              Join
            </Button>
          </div>
        </>
      )}

      {event.startedAt && (
        <div {...stylex.props(styles.eventText)}>{formatEventDateTime()}</div>
      )}

      {children}

      {event.description && (
        <>
          <div {...stylex.props(styles.divider)} />
          <div {...stylex.props(styles.description)}>
            {renderDescriptionWithLinks(event.description)}
          </div>
        </>
      )}
    </div>
  );
}

const TRAILING_LINK_PUNCTUATION = ".,!?;:)]}";

function parseLinkCandidate(
  candidate: string,
): { url: string; suffix: string } | null {
  let url = candidate;
  let suffix = "";

  while (url.length > 0) {
    try {
      new URL(url);
      return { url, suffix };
    } catch {
      const lastChar = url[url.length - 1];
      if (!lastChar || !TRAILING_LINK_PUNCTUATION.includes(lastChar)) {
        return null;
      }
      suffix = `${lastChar}${suffix}`;
      url = url.slice(0, -1);
    }
  }

  return null;
}

function renderDescriptionWithLinks(description: string): React.ReactNode {
  const nodes: React.ReactNode[] = [];
  let lastIndex = 0;
  let linkIndex = 0;
  const urlPattern = /https?:\/\/[^\s<>"'`]+/gi;

  for (const match of description.matchAll(urlPattern)) {
    if (match.index === undefined) {
      continue;
    }

    const rawMatch = match[0];
    const start = match.index;
    const end = start + rawMatch.length;

    if (start > lastIndex) {
      nodes.push(description.slice(lastIndex, start));
    }

    const parsedLink = parseLinkCandidate(rawMatch);
    if (!parsedLink) {
      nodes.push(rawMatch);
      lastIndex = end;
      continue;
    }

    const { url, suffix } = parsedLink;
    nodes.push(
      <a
        key={`description-link-${linkIndex}`}
        href={url}
        {...stylex.props(styles.link)}
        onClick={(e) => {
          e.preventDefault();
          void openerCommands.openUrl(url, null);
        }}
      >
        {url}
      </a>,
    );
    if (suffix) {
      nodes.push(suffix);
    }

    linkIndex += 1;
    lastIndex = end;
  }

  if (lastIndex < description.length) {
    nodes.push(description.slice(lastIndex));
  }

  return nodes.length > 0 ? nodes : description;
}

const styles = stylex.create({
  content: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
    padding: "1rem",
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    overflowWrap: "break-word",
    userSelect: {
      default: "text",
      ":is(*) *": "text",
    },
    whiteSpace: "pre-wrap",
  },
  divider: {
    backgroundColor: colors.accent,
    height: "1px",
  },
  event: {
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
  },
  eventRow: {
    alignItems: "center",
    color: colors.mutedForeground,
    display: "flex",
    fontSize: "0.875rem",
    gap: "0.5rem",
  },
  eventText: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  joinButton: {
    flexShrink: 0,
  },
  link: {
    color: {
      default: "inherit",
      ":hover": colors.foreground,
    },
    cursor: "pointer",
    textDecorationLine: "underline",
    transitionDuration: "150ms",
    transitionProperty: "color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  meetingDetails: {
    minWidth: 0,
  },
  meetingRow: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    justifyContent: "space-between",
  },
  panel: {
    maxHeight: "80vh",
    minHeight: 0,
    overflowX: "hidden",
    overflowY: "auto",
    overscrollBehavior: "contain",
    scrollbarColor:
      "hsl(var(--muted-foreground, 25 5% 45%) / 0.18) transparent",
  },
  popover: {
    overflow: "hidden",
    width: "21.25rem",
  },
  rowIcon: {
    color: colors.mutedForeground,
    flexShrink: 0,
  },
  title: {
    color: colors.foreground,
    fontSize: "1rem",
    fontWeight: 500,
  },
  trigger: {
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
  triggerOpen: {
    backgroundColor: colors.muted,
    color: colors.foreground,
  },
  truncate: {
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
});

export { styles as metadataStyles };
