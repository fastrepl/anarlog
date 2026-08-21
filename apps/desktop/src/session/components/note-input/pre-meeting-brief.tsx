import { Trans, useLingui } from "@lingui/react/macro";
import {
  CalendarBlank,
  MapPin,
  Sparkle,
  UsersThree,
} from "@phosphor-icons/react";

import { cn, safeParseDate } from "@anlg/utils";

import { useNow } from "~/calendar/hooks";
import { useSessionCalendarEvent } from "~/calendar/queries";
import { usePastSessionNotes } from "~/session/insights/past-notes";
import {
  compactBriefText,
  getPreMeetingBriefFacts,
} from "~/session/insights/pre-meeting";
import { useConfigValue } from "~/shared/config";

const MAX_DESCRIPTION_LENGTH = 240;
const AFTER_START_GRACE_MS = 5 * 60 * 1000;

export function PreMeetingBrief({
  sessionId,
  enabled = true,
}: {
  sessionId: string;
  enabled?: boolean;
}) {
  const { i18n } = useLingui();
  const event = useSessionCalendarEvent(sessionId, { enabled });
  const now = useNow();
  const timezone = useConfigValue("timezone") || undefined;
  const visible = enabled && shouldShowPreMeetingBrief(event, now.getTime());
  const pastNotes = usePastSessionNotes(sessionId, { enabled: visible });

  const participantNames = [
    ...new Set(
      (event?.participants ?? [])
        .filter((participant) => participant.is_current_user !== true)
        .map(
          (participant) =>
            participant.name?.trim() || participant.email?.trim() || "",
        )
        .filter(Boolean),
    ),
  ].slice(0, 5);

  if (!visible || !event) {
    return null;
  }

  const latestNote =
    pastNotes.notes.find((note) => note.relationship === "same_series") ?? null;
  const facts = latestNote ? getPreMeetingBriefFacts(latestNote) : [];
  const description = compactBriefText(
    event.description ?? "",
    MAX_DESCRIPTION_LENGTH,
  );
  const location = compactBriefText(event.location ?? "", 120);
  const eventTime = formatEventTime(
    event.started_at,
    event.ended_at,
    timezone,
    i18n.locale,
  );
  const relatedDate = latestNote
    ? formatBriefDate(latestNote.occurredAt, timezone, i18n.locale)
    : "";

  return (
    <section
      data-pre-meeting-brief
      className={cn([
        "border-border/70 bg-muted/35 mx-1 mb-3 rounded-xl border px-4 py-3",
        "text-sm",
      ])}
    >
      <div className="flex items-center gap-2">
        <Sparkle aria-hidden className="text-primary size-4 shrink-0" />
        <h2 className="font-medium">
          <Trans>Pre-meeting brief</Trans>
        </h2>
      </div>

      <div className="text-muted-foreground mt-2 flex flex-wrap gap-x-4 gap-y-1.5 text-xs">
        {eventTime ? (
          <span className="flex min-w-0 items-center gap-1.5">
            <CalendarBlank aria-hidden className="size-3.5 shrink-0" />
            <span>{eventTime}</span>
          </span>
        ) : null}
        {location ? (
          <span className="flex min-w-0 items-center gap-1.5">
            <MapPin aria-hidden className="size-3.5 shrink-0" />
            <span className="max-w-64 truncate">{location}</span>
          </span>
        ) : null}
        {participantNames.length > 0 ? (
          <span className="flex min-w-0 items-center gap-1.5">
            <UsersThree aria-hidden className="size-3.5 shrink-0" />
            <span className="max-w-80 truncate">
              {participantNames.join(", ")}
            </span>
          </span>
        ) : null}
      </div>

      {description ? (
        <p className="text-muted-foreground mt-2 line-clamp-2 leading-relaxed">
          {description}
        </p>
      ) : null}

      <div className="border-border/60 mt-3 border-t pt-3">
        {latestNote ? (
          <>
            <p className="text-muted-foreground mb-1.5 text-xs font-medium">
              <Trans>Last meeting</Trans>
              {relatedDate ? ` · ${relatedDate}` : ""}
            </p>
            <ul className="space-y-1.5">
              {facts.map((fact) => (
                <li key={fact} className="flex gap-2 leading-relaxed">
                  <span
                    aria-hidden
                    className="bg-muted-foreground/60 mt-[0.55rem] size-1 shrink-0 rounded-full"
                  />
                  <span>{fact}</span>
                </li>
              ))}
            </ul>
          </>
        ) : (
          <p className="text-muted-foreground leading-relaxed">
            <Trans>
              No previous meeting summary is available. Calendar details are
              shown above.
            </Trans>
          </p>
        )}
      </div>
    </section>
  );
}

export function shouldShowPreMeetingBrief(
  event: {
    started_at?: string;
    ended_at?: string;
    is_all_day?: boolean;
  } | null,
  nowMs: number,
): boolean {
  if (!event || event.is_all_day) {
    return false;
  }

  const startMs = Date.parse(event.started_at ?? "");
  if (!Number.isFinite(startMs)) {
    return false;
  }
  if (startMs > nowMs) {
    return true;
  }

  const endMs = Date.parse(event.ended_at ?? "");
  const hideAfterMs = Number.isFinite(endMs)
    ? Math.max(endMs, startMs + AFTER_START_GRACE_MS)
    : startMs + AFTER_START_GRACE_MS;
  return hideAfterMs > nowMs;
}

function formatEventTime(
  startedAt: string,
  endedAt: string,
  timezone: string | undefined,
  locale: string,
): string {
  const rawStart = safeParseDate(startedAt);
  if (!rawStart) {
    return "";
  }

  const rawEnd = safeParseDate(endedAt);
  const dateOptions = {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  } satisfies Intl.DateTimeFormatOptions;
  const dateTimeOptions = {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  } satisfies Intl.DateTimeFormatOptions;
  const timeOptions = {
    hour: "numeric",
    minute: "2-digit",
  } satisfies Intl.DateTimeFormatOptions;
  const startText = formatDateTime(rawStart, locale, timezone, dateTimeOptions);
  if (!rawEnd) {
    return startText;
  }

  const sameDay =
    formatDateTime(rawStart, locale, timezone, dateOptions) ===
    formatDateTime(rawEnd, locale, timezone, dateOptions);
  const endText = formatDateTime(
    rawEnd,
    locale,
    timezone,
    sameDay ? timeOptions : dateTimeOptions,
  );
  return `${startText}–${endText}`;
}

function formatBriefDate(
  value: string,
  timezone: string | undefined,
  locale: string,
): string {
  const date = safeParseDate(value);
  return date
    ? formatDateTime(date, locale, timezone, {
        year: "numeric",
        month: "short",
        day: "numeric",
      })
    : "";
}

function formatDateTime(
  date: Date,
  locale: string,
  timezone: string | undefined,
  options: Intl.DateTimeFormatOptions,
): string {
  try {
    return new Intl.DateTimeFormat(locale || undefined, {
      ...options,
      timeZone: timezone,
    }).format(date);
  } catch {
    return new Intl.DateTimeFormat(undefined, options).format(date);
  }
}
