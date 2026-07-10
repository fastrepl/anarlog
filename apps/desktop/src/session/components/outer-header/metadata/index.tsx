import { CalendarIcon, MapPinIcon, VideoIcon } from "lucide-react";
import { forwardRef, type ReactElement, useState } from "react";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import { Button } from "@hypr/ui/components/ui/button";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import { cn, safeFormat, safeParseDate, TZDate } from "@hypr/utils";

import { DateEditor } from "./date";
import { ParticipantsDisplay } from "./participants";

import { useSessionEvent } from "~/session/hooks/useSessionEvent";
import { useConfigValue } from "~/shared/config";

export function MetadataButton({
  sessionId,
  renderTrigger,
}: {
  sessionId: string;
  renderTrigger?: (props: { open: boolean; label: string }) => ReactElement;
}) {
  const [open, setOpen] = useState(false);
  const sessionEvent = useSessionEvent(sessionId);
  const label = sessionEvent ? "Open event metadata" : "Open note metadata";

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        {renderTrigger ? (
          renderTrigger({ open, label })
        ) : (
          <TriggerInner label={label} open={open} />
        )}
      </PopoverTrigger>
      <PopoverContent
        variant="app"
        align="end"
        className="w-85 overflow-hidden"
      >
        <AppFloatingPanel className="scrollbar-soft max-h-[80vh] min-h-0 overflow-x-hidden overflow-y-auto overscroll-contain">
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
      className={cn([
        "size-7 rounded-full",
        "text-muted-foreground hover:bg-accent hover:text-foreground",
        open && "bg-muted text-foreground",
      ])}
    >
      <CalendarIcon size={16} />
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
    <div className="flex flex-col gap-4 p-4">
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
    <div className="flex flex-col gap-3">
      <div className="text-foreground text-base font-medium">
        {event.title || "Untitled Event"}
      </div>

      <div className="bg-accent h-px" />

      {shouldShowLocation && (
        <>
          <div className="text-muted-foreground flex items-center gap-2 text-sm">
            <MapPinIcon size={16} className="text-muted-foreground shrink-0" />
            <span>{event.location}</span>
          </div>
        </>
      )}

      {event.meetingLink && (
        <>
          <div className="flex items-center justify-between gap-2">
            <div className="text-muted-foreground flex min-w-0 items-center gap-2 text-sm">
              <VideoIcon size={16} className="text-muted-foreground shrink-0" />
              <span className="truncate">
                {meetingDomain || "Meeting link"}
              </span>
            </div>
            <Button
              size="sm"
              variant="default"
              className="shrink-0"
              onClick={handleJoinMeeting}
            >
              Join
            </Button>
          </div>
        </>
      )}

      {event.startedAt && (
        <div className="text-muted-foreground text-sm">
          {formatEventDateTime()}
        </div>
      )}

      {children}

      {event.description && (
        <>
          <div className="bg-accent h-px" />
          <div className="select-text-deep text-muted-foreground text-sm break-words whitespace-pre-wrap">
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
        className="hover:text-foreground cursor-pointer underline transition-colors"
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
