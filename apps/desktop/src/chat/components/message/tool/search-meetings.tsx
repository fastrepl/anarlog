import { MagnifyingGlass } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useCallback, useMemo } from "react";

import { colors, media } from "@anlg/design-system/tokens.stylex";
import { Card, CardContent } from "@anlg/ui/components/ui/card";
import {
  Carousel,
  CarouselContent,
  CarouselItem,
  CarouselNext,
  CarouselPrevious,
} from "@anlg/ui/components/ui/carousel";

import { useToolState } from "./shared";

import { trackAnalyticsEvent } from "~/analytics";
import { Disclosure } from "~/chat/components/message/shared";
import { ToolRenderer } from "~/chat/components/message/types";
import { useTabs } from "~/store/zustand/tabs";

type Renderer = ToolRenderer<"tool-search_meetings">;
type Part = Parameters<Renderer>[0]["part"];
type MeetingSearchResult = {
  id: string;
  title: string;
  excerpt: string;
  score: number;
  created_at: number | string;
};

function parseMeetingSearchResults(output: unknown): MeetingSearchResult[] {
  if (!output || typeof output !== "object") {
    return [];
  }

  const record = output as { results?: unknown; meetings?: unknown };
  const results = Array.isArray(record.results)
    ? record.results
    : Array.isArray(record.meetings)
      ? record.meetings
      : null;
  if (!results) {
    return [];
  }

  return results.flatMap((result): MeetingSearchResult[] => {
    if (!result || typeof result !== "object") {
      return [];
    }

    const { id, title, excerpt, score, created_at, started_at } = result as {
      id?: unknown;
      title?: unknown;
      excerpt?: unknown;
      score?: unknown;
      created_at?: unknown;
      started_at?: unknown;
    };
    if (typeof id !== "string") {
      return [];
    }

    return [
      {
        id,
        title: typeof title === "string" ? title : "Untitled",
        excerpt: typeof excerpt === "string" ? excerpt : "",
        score: typeof score === "number" ? score : 0,
        created_at:
          typeof started_at === "string" && started_at
            ? started_at
            : typeof created_at === "number" || typeof created_at === "string"
              ? created_at
              : 0,
      },
    ];
  });
}

function formatSearchInput(input: Part["input"] | undefined): {
  titleQuery: string;
  details: string[];
} {
  if (!input) {
    return { titleQuery: "meetings", details: [] };
  }

  const details: string[] = [];
  const rawQuery = typeof input.query === "string" ? input.query.trim() : "";
  const titleQuery = rawQuery || "meetings";

  if (!rawQuery) {
    details.push("Query: none");
  } else {
    details.push(`Query: ${rawQuery}`);
  }

  const createdAt = input.filters?.created_at;
  if (createdAt?.kind === "relative") {
    details.push(
      `Date: recent ${createdAt.recent_days} day(s), including today`,
    );
  } else if (createdAt?.kind === "absolute") {
    const bounds = [
      createdAt.gte != null
        ? `gte ${new Date(createdAt.gte).toLocaleString()}`
        : null,
      createdAt.lte != null
        ? `lte ${new Date(createdAt.lte).toLocaleString()}`
        : null,
      createdAt.gt != null
        ? `gt ${new Date(createdAt.gt).toLocaleString()}`
        : null,
      createdAt.lt != null
        ? `lt ${new Date(createdAt.lt).toLocaleString()}`
        : null,
      createdAt.eq != null
        ? `eq ${new Date(createdAt.eq).toLocaleString()}`
        : null,
    ].filter(Boolean);

    if (bounds.length > 0) {
      details.push(`Date: ${bounds.join(", ")}`);
    }
  }

  if (typeof input.limit === "number") {
    details.push(`Limit: ${input.limit}`);
  }

  return { titleQuery, details };
}

export const ToolSearchMeetings: Renderer = ({ part }) => {
  const { running: disabled } = useToolState(part);

  return (
    <Disclosure
      icon={<MagnifyingGlass {...stylex.props(styles.smallIcon)} />}
      title={getTitle(part)}
      disabled={disabled}
    >
      <RenderContent part={part} />
    </Disclosure>
  );
};

const getTitle = (part: Part) => {
  const { titleQuery } = formatSearchInput(part.input);

  if (part.state === "input-streaming") {
    return "Preparing search...";
  }
  if (part.state === "input-available") {
    return `Searching for: ${titleQuery}`;
  }
  if (part.state === "output-available") {
    return `Searched for: ${titleQuery}`;
  }
  if (part.state === "output-error") {
    return part.input ? `Search failed: ${titleQuery}` : "Search failed";
  }
  return "Search";
};

function RenderContent({ part }: { part: Part }) {
  const { details } = formatSearchInput(part.input);

  if (part.state === "output-available") {
    const results = parseMeetingSearchResults(part.output);

    if (!results || results.length === 0) {
      return (
        <div {...stylex.props(styles.content)}>
          {details.length > 0 && (
            <div {...stylex.props(styles.details)}>
              {details.map((detail) => (
                <div key={detail}>{detail}</div>
              ))}
            </div>
          )}
          <div {...stylex.props(styles.emptyResults)}>No results found</div>
        </div>
      );
    }

    return (
      <div {...stylex.props(styles.content)}>
        {details.length > 0 && (
          <div {...stylex.props(styles.details)}>
            {details.map((detail) => (
              <div key={detail}>{detail}</div>
            ))}
          </div>
        )}
        <div {...stylex.props(styles.carouselContainer)}>
          <Carousel sx={styles.carousel} opts={{ align: "start" }}>
            <CarouselContent sx={styles.carouselContent}>
              {results.map((result, index: number) => (
                <CarouselItem key={result.id || index} sx={styles.carouselItem}>
                  <Card sx={styles.resultCard}>
                    <CardContent sx={styles.resultCardContent}>
                      <RenderMeeting result={result} />
                    </CardContent>
                  </Card>
                </CarouselItem>
              ))}
            </CarouselContent>
            <CarouselPrevious sx={[styles.carouselButton, styles.previous]} />
            <CarouselNext sx={[styles.carouselButton, styles.next]} />
          </Carousel>
        </div>
      </div>
    );
  }

  if (part.state === "output-error") {
    return <div {...stylex.props(styles.error)}>Error: {part.errorText}</div>;
  }

  return details.length > 0 ? (
    <div {...stylex.props(styles.details)}>
      {details.map((detail) => (
        <div key={detail}>{detail}</div>
      ))}
    </div>
  ) : null;
}

function RenderMeeting({ result }: { result: MeetingSearchResult }) {
  const { id: sessionId } = result;
  const openNew = useTabs((state) => state.openNew);

  const handleClick = useCallback(() => {
    trackAnalyticsEvent("search_result_opened", {
      entry_point: "chat_search",
      result_type: "session",
    });
    openNew({ type: "sessions", id: sessionId });
  }, [openNew, sessionId]);

  const dateLabel = useMemo(() => {
    if (!result.created_at) return null;
    return new Date(result.created_at).toLocaleString();
  }, [result.created_at]);

  return (
    <button
      type="button"
      onClick={handleClick}
      {...stylex.props(styles.result)}
    >
      <span {...stylex.props(styles.resultTitle)}>
        {result.title || "Untitled"}
      </span>
      {dateLabel && (
        <span {...stylex.props(styles.resultDate)}>{dateLabel}</span>
      )}
      <span {...stylex.props(styles.resultExcerpt)}>
        {result.excerpt || "No excerpt available"}
      </span>
    </button>
  );
}

const styles = stylex.create({
  smallIcon: {
    height: "0.75rem",
    width: "0.75rem",
  },
  content: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
  },
  details: {
    color: colors.mutedForeground,
    display: "flex",
    flexDirection: "column",
    fontSize: "0.6875rem",
    gap: "0.125rem",
  },
  emptyResults: {
    alignItems: "center",
    color: colors.mutedForeground,
    display: "flex",
    fontSize: "0.75rem",
    justifyContent: "center",
    lineHeight: "1rem",
    paddingBlock: "0.5rem",
  },
  carouselContainer: {
    marginInline: "-0.25rem",
    position: "relative",
  },
  carousel: {
    width: "100%",
  },
  carouselContent: {
    marginLeft: "-0.5rem",
  },
  carouselItem: {
    flexBasis: {
      default: "100%",
      [media.sm]: "50%",
      [media.md]: "33.333333%",
    },
    paddingLeft: "0.25rem",
  },
  resultCard: {
    backgroundColor: colors.muted,
    height: "100%",
  },
  resultCardContent: {
    paddingBlock: "0.125rem",
    paddingInline: "0.5rem",
  },
  carouselButton: {
    backgroundColor: {
      default: colors.muted,
      ":hover": colors.accent,
    },
    height: "1.5rem",
    width: "1.5rem",
  },
  previous: {
    left: "-1rem",
  },
  next: {
    right: "-1rem",
  },
  error: {
    color: "oklch(63.7% 0.237 25.331)",
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
  },
  result: {
    display: "flex",
    flexDirection: "column",
    fontSize: "0.75rem",
    gap: "0.25rem",
    lineHeight: "1rem",
    textAlign: "left",
    width: "100%",
  },
  resultTitle: {
    fontWeight: 500,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  resultDate: {
    color: colors.mutedForeground,
    fontSize: "0.6875rem",
    fontVariantNumeric: "tabular-nums",
  },
  resultExcerpt: {
    WebkitBoxOrient: "vertical",
    WebkitLineClamp: 3,
    color: colors.mutedForeground,
    display: "-webkit-box",
    overflow: "hidden",
    overflowWrap: "break-word",
  },
});
