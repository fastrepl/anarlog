import { SearchIcon } from "lucide-react";
import { useCallback, useMemo } from "react";

import { Card, CardContent } from "@hypr/ui/components/ui/card";
import {
  Carousel,
  CarouselContent,
  CarouselItem,
  CarouselNext,
  CarouselPrevious,
} from "@hypr/ui/components/ui/carousel";

import { useToolState } from "./shared";

import { Disclosure } from "~/chat/components/message/shared";
import { ToolRenderer } from "~/chat/components/message/types";
import * as main from "~/store/tinybase/store/main";
import { useTabs } from "~/store/zustand/tabs";

type Renderer = ToolRenderer<"tool-search_sessions">;
type Part = Parameters<Renderer>[0]["part"];
type SearchResult = {
  id: string;
  title: string;
  excerpt: string;
  score: number;
  created_at: number;
};

function parseSearchResults(output: unknown): SearchResult[] {
  if (!output || typeof output !== "object" || !("results" in output)) {
    return [];
  }

  const { results } = output as { results?: unknown };
  if (!Array.isArray(results)) {
    return [];
  }

  return results.flatMap((result): SearchResult[] => {
    if (!result || typeof result !== "object") {
      return [];
    }

    const { id, title, excerpt, score, created_at } = result as {
      id?: unknown;
      title?: unknown;
      excerpt?: unknown;
      score?: unknown;
      created_at?: unknown;
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
        created_at: typeof created_at === "number" ? created_at : 0,
      },
    ];
  });
}

function formatSearchInput(input: Part["input"] | undefined): {
  titleQuery: string;
  details: string[];
} {
  if (!input) {
    return { titleQuery: "sessions", details: [] };
  }

  const details: string[] = [];
  const rawQuery = typeof input.query === "string" ? input.query.trim() : "";
  const titleQuery = rawQuery || "sessions";

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

export const ToolSearchSessions: Renderer = ({ part }) => {
  const { running: disabled } = useToolState(part);

  return (
    <Disclosure
      icon={<SearchIcon className="h-3 w-3" />}
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
    const results = parseSearchResults(part.output);

    if (!results || results.length === 0) {
      return (
        <div className="flex flex-col gap-2">
          {details.length > 0 && (
            <div className="text-muted-foreground flex flex-col gap-0.5 text-[11px]">
              {details.map((detail) => (
                <div key={detail}>{detail}</div>
              ))}
            </div>
          )}
          <div className="text-muted-foreground flex items-center justify-center py-2 text-xs">
            No results found
          </div>
        </div>
      );
    }

    return (
      <div className="flex flex-col gap-2">
        {details.length > 0 && (
          <div className="text-muted-foreground flex flex-col gap-0.5 text-[11px]">
            {details.map((detail) => (
              <div key={detail}>{detail}</div>
            ))}
          </div>
        )}
        <div className="relative -mx-1">
          <Carousel className="w-full" opts={{ align: "start" }}>
            <CarouselContent className="-ml-2">
              {results.map((result, index: number) => (
                <CarouselItem
                  key={result.id || index}
                  className="basis-full pl-1 sm:basis-1/2 lg:basis-1/3"
                >
                  <Card className="h-full bg-neutral-50">
                    <CardContent className="px-2 py-0.5">
                      <RenderSession result={result} />
                    </CardContent>
                  </Card>
                </CarouselItem>
              ))}
            </CarouselContent>
            <CarouselPrevious className="-left-4 h-6 w-6 bg-neutral-100 hover:bg-neutral-200" />
            <CarouselNext className="-right-4 h-6 w-6 bg-neutral-100 hover:bg-neutral-200" />
          </Carousel>
        </div>
      </div>
    );
  }

  if (part.state === "output-error") {
    return <div className="text-sm text-red-500">Error: {part.errorText}</div>;
  }

  return details.length > 0 ? (
    <div className="text-muted-foreground flex flex-col gap-0.5 text-[11px]">
      {details.map((detail) => (
        <div key={detail}>{detail}</div>
      ))}
    </div>
  ) : null;
}

function RenderSession({ result }: { result: SearchResult }) {
  const { id: sessionId } = result;
  const session = main.UI.useRow("sessions", sessionId, main.STORE_ID);
  const openNew = useTabs((state) => state.openNew);

  const handleClick = useCallback(() => {
    openNew({ type: "sessions", id: sessionId });
  }, [openNew, sessionId]);

  const dateLabel = useMemo(() => {
    if (!result.created_at) return null;
    return new Date(result.created_at).toLocaleString();
  }, [result.created_at]);

  if (!session) {
    return (
      <div className="text-muted-foreground text-xs italic">
        Session unavailable
      </div>
    );
  }

  return (
    <button
      type="button"
      onClick={handleClick}
      className="flex w-full flex-col gap-1 text-left text-xs"
    >
      <span className="truncate font-medium">{result.title || "Untitled"}</span>
      {dateLabel && (
        <span className="text-[11px] text-neutral-400 tabular-nums">
          {dateLabel}
        </span>
      )}
      <span className="text-muted-foreground line-clamp-3 break-words">
        {result.excerpt || "No excerpt available"}
      </span>
    </button>
  );
}
