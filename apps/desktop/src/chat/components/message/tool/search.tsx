import { SearchIcon } from "lucide-react";
import { useCallback } from "react";

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

    const { id } = result as { id?: unknown };
    if (typeof id !== "string") {
      return [];
    }

    return [{ id }];
  });
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
  if (part.state === "input-streaming") {
    return "Preparing search...";
  }
  if (part.state === "input-available") {
    return `Searching for: ${part.input.query}`;
  }
  if (part.state === "output-available") {
    return `Searched for: ${part.input.query}`;
  }
  if (part.state === "output-error") {
    return part.input ? `Search failed: ${part.input.query}` : "Search failed";
  }
  return "Search";
};

function RenderContent({ part }: { part: Part }) {
  if (part.state === "output-available") {
    const results = parseSearchResults(part.output);

    if (!results || results.length === 0) {
      return (
        <div className="text-muted-foreground flex items-center justify-center py-2 text-xs">
          No results found
        </div>
      );
    }

    return (
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
                    <RenderSession sessionId={result.id} />
                  </CardContent>
                </Card>
              </CarouselItem>
            ))}
          </CarouselContent>
          <CarouselPrevious className="-left-4 h-6 w-6 bg-neutral-100 hover:bg-neutral-200" />
          <CarouselNext className="-right-4 h-6 w-6 bg-neutral-100 hover:bg-neutral-200" />
        </Carousel>
      </div>
    );
  }

  if (part.state === "output-error") {
    return <div className="text-sm text-red-500">Error: {part.errorText}</div>;
  }

  return null;
}

function RenderSession({ sessionId }: { sessionId: string }) {
  const session = main.UI.useRow("sessions", sessionId, main.STORE_ID);
  const enhancedNoteIds = main.UI.useSliceRowIds(
    main.INDEXES.enhancedNotesBySession,
    sessionId,
    main.STORE_ID,
  );
  const firstEnhancedNoteId = enhancedNoteIds?.[0];
  const enhancedNoteContent = main.UI.useCell(
    "enhanced_notes",
    firstEnhancedNoteId ?? "",
    "content",
    main.STORE_ID,
  );
  const openNew = useTabs((state) => state.openNew);

  const handleClick = useCallback(() => {
    openNew({ type: "sessions", id: sessionId });
  }, [openNew, sessionId]);

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
      <span className="truncate font-medium">
        {session.title || "Untitled"}
      </span>
      <span className="text-muted-foreground truncate">
        {enhancedNoteContent ?? session.raw_md}
      </span>
    </button>
  );
}
