import DOMPurify from "dompurify";
import { Building2Icon, FileTextIcon, UserIcon } from "lucide-react";
import { useMemo } from "react";

import { cn } from "@hypr/utils";

import type { SearchResult } from "~/search/contexts/ui";

const TYPE_ICONS = {
  session: FileTextIcon,
  human: UserIcon,
  organization: Building2Icon,
};

interface ResultItemProps {
  result: SearchResult;
  onClick: () => void;
  isSelected?: boolean;
}

export function ResultItem({ result, onClick, isSelected }: ResultItemProps) {
  const Icon = TYPE_ICONS[result.type] || FileTextIcon;
  const sanitizedTitle = useMemo(
    () =>
      DOMPurify.sanitize(result.titleHighlighted, {
        ALLOWED_TAGS: ["mark"],
      }),
    [result.titleHighlighted],
  );

  const sanitizedContent = useMemo(
    () =>
      DOMPurify.sanitize(result.contentHighlighted, {
        ALLOWED_TAGS: ["mark"],
      }),
    [result.contentHighlighted],
  );

  return (
    <button
      data-result-id={result.id}
      onClick={onClick}
      className={cn([
        "flex w-full items-start gap-3 p-3",
        "rounded-lg text-left",
        "transition-colors hover:bg-neutral-100",
        isSelected && "bg-neutral-100",
      ])}
    >
      <div className="mt-0.5 shrink-0">
        <Icon className="h-4 w-4 text-neutral-400" />
      </div>
      <div className="min-w-0 flex-1">
        <div
          className="truncate font-medium text-neutral-900"
          dangerouslySetInnerHTML={{ __html: sanitizedTitle }}
        />
        {result.content && (
          <div
            className="mt-0.5 truncate text-sm text-neutral-500"
            dangerouslySetInnerHTML={{ __html: sanitizedContent }}
          />
        )}
      </div>
    </button>
  );
}
