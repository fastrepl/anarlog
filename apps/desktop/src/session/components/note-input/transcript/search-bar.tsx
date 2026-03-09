import {
  ALargeSmallIcon,
  ChevronDownIcon,
  ChevronUpIcon,
  ReplaceAllIcon,
  ReplaceIcon,
  WholeWordIcon,
  XIcon,
} from "lucide-react";
import { useEffect, useRef } from "react";

import { Kbd } from "@hypr/ui/components/ui/kbd";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import { useTranscriptSearch } from "./search-context";

function ToggleButton({
  active,
  onClick,
  tooltip,
  children,
}: {
  active: boolean;
  onClick: () => void;
  tooltip: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          onClick={onClick}
          className={cn([
            "rounded-sm p-0.5 transition-colors",
            active
              ? "bg-neutral-300 text-neutral-700"
              : "text-neutral-400 hover:bg-neutral-200 hover:text-neutral-500",
          ])}
        >
          {children}
        </button>
      </TooltipTrigger>
      <TooltipContent side="bottom" className="flex items-center gap-2">
        {tooltip}
      </TooltipContent>
    </Tooltip>
  );
}

function IconButton({
  onClick,
  disabled,
  tooltip,
  children,
}: {
  onClick: () => void;
  disabled?: boolean;
  tooltip: React.ReactNode;
  children: React.ReactNode;
}) {
  const btn = (
    <button
      onClick={onClick}
      disabled={disabled}
      className={cn([
        "rounded-sm p-0.5 transition-colors",
        disabled
          ? "cursor-not-allowed text-neutral-300"
          : "text-neutral-500 hover:bg-neutral-200",
      ])}
    >
      {children}
    </button>
  );

  if (disabled) return btn;

  return (
    <Tooltip>
      <TooltipTrigger asChild>{btn}</TooltipTrigger>
      <TooltipContent side="bottom" className="flex items-center gap-2">
        {tooltip}
      </TooltipContent>
    </Tooltip>
  );
}

export function SearchBar() {
  const search = useTranscriptSearch();
  const searchInputRef = useRef<HTMLInputElement>(null);
  const replaceInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    searchInputRef.current?.focus();
  }, []);

  useEffect(() => {
    if (search?.showReplace) {
      replaceInputRef.current?.focus();
    }
  }, [search?.showReplace]);

  if (!search) {
    return null;
  }

  const {
    query,
    setQuery,
    currentMatchIndex,
    totalMatches,
    onNext,
    onPrev,
    close,
    caseSensitive,
    wholeWord,
    showReplace,
    replaceQuery,
    toggleCaseSensitive,
    toggleWholeWord,
    toggleReplace,
    setReplaceQuery,
    replaceCurrent,
    replaceAll,
  } = search;

  const handleSearchKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      if (e.shiftKey) {
        onPrev();
      } else {
        onNext();
      }
    }
  };

  const handleReplaceKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      if (e.metaKey || e.ctrlKey) {
        replaceAll();
      } else {
        replaceCurrent();
      }
    }
  };

  const displayCount =
    totalMatches > 0 ? `${currentMatchIndex + 1}/${totalMatches}` : "0/0";

  return (
    <div className="flex flex-col gap-1">
      <div className="flex h-7 items-center gap-1.5 rounded-lg bg-neutral-100 px-2">
        <input
          ref={searchInputRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleSearchKeyDown}
          placeholder="Search..."
          className="h-full min-w-0 flex-1 bg-transparent text-xs placeholder:text-neutral-400 focus:outline-hidden"
        />
        <div className="flex items-center gap-0.5">
          <ToggleButton
            active={caseSensitive}
            onClick={toggleCaseSensitive}
            tooltip="Match case"
          >
            <ALargeSmallIcon className="size-3.5" />
          </ToggleButton>
          <ToggleButton
            active={wholeWord}
            onClick={toggleWholeWord}
            tooltip="Match whole word"
          >
            <WholeWordIcon className="size-3.5" />
          </ToggleButton>
          <ToggleButton
            active={showReplace}
            onClick={toggleReplace}
            tooltip={
              <>
                <span>Replace</span>
                <Kbd className="animate-kbd-press">⌘ H</Kbd>
              </>
            }
          >
            <ReplaceIcon className="size-3.5" />
          </ToggleButton>
        </div>
        <span className="text-[10px] whitespace-nowrap text-neutral-400 tabular-nums">
          {displayCount}
        </span>
        <div className="flex items-center">
          <IconButton
            onClick={onPrev}
            disabled={totalMatches === 0}
            tooltip={
              <>
                <span>Previous match</span>
                <Kbd className="animate-kbd-press">⇧ ↵</Kbd>
              </>
            }
          >
            <ChevronUpIcon className="size-3.5" />
          </IconButton>
          <IconButton
            onClick={onNext}
            disabled={totalMatches === 0}
            tooltip={
              <>
                <span>Next match</span>
                <Kbd className="animate-kbd-press">↵</Kbd>
              </>
            }
          >
            <ChevronDownIcon className="size-3.5" />
          </IconButton>
        </div>
        <IconButton
          onClick={close}
          tooltip={
            <>
              <span>Close</span>
              <Kbd className="animate-kbd-press">Esc</Kbd>
            </>
          }
        >
          <XIcon className="size-3.5" />
        </IconButton>
      </div>

      {showReplace && (
        <div className="flex h-7 items-center gap-1.5 rounded-lg bg-neutral-100 px-2">
          <input
            ref={replaceInputRef}
            type="text"
            value={replaceQuery}
            onChange={(e) => setReplaceQuery(e.target.value)}
            onKeyDown={handleReplaceKeyDown}
            placeholder="Replace with..."
            className="h-full min-w-0 flex-1 bg-transparent text-xs placeholder:text-neutral-400 focus:outline-hidden"
          />
          <div className="flex items-center gap-0.5">
            <IconButton
              onClick={replaceCurrent}
              tooltip={
                <>
                  <span>Replace</span>
                  <Kbd className="animate-kbd-press">↵</Kbd>
                </>
              }
            >
              <ReplaceIcon className="size-3.5" />
            </IconButton>
            <IconButton
              onClick={replaceAll}
              tooltip={
                <>
                  <span>Replace all</span>
                  <Kbd className="animate-kbd-press">⌘ ↵</Kbd>
                </>
              }
            >
              <ReplaceAllIcon className="size-3.5" />
            </IconButton>
          </div>
        </div>
      )}
    </div>
  );
}
