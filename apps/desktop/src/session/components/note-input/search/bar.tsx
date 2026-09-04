import { Trans, useLingui } from "@lingui/react/macro";
import { platform } from "@tauri-apps/plugin-os";
import { useEffect, useRef } from "react";

import type { NoteEditorRef } from "@anlg/editor/note";
import {
  CaretDown,
  CaretUp,
  Repeat,
  Swap,
  TextAa,
  Textbox,
  X,
} from "@anlg/ui/components/icons";
import { Kbd } from "@anlg/ui/components/ui/kbd";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@anlg/ui/components/ui/tooltip";
import { cn } from "@anlg/utils";

import { useSearch } from "./context";

import { useMountEffect } from "~/shared/hooks/useMountEffect";

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
              ? "bg-accent text-muted-foreground"
              : "text-muted-foreground hover:bg-accent hover:text-muted-foreground",
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
          ? "text-muted-foreground/70 cursor-not-allowed"
          : "text-muted-foreground hover:bg-accent",
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

export function SearchBar({
  editorRef,
  allowReplace = true,
}: {
  editorRef?: React.RefObject<NoteEditorRef | null>;
  allowReplace?: boolean;
}) {
  const { t } = useLingui();
  const search = useSearch();
  const primaryModifier = platform() === "macos" ? "⌘" : "Ctrl";
  const searchInputRef = useRef<HTMLInputElement>(null);
  const replaceInputRef = useRef<HTMLInputElement>(null);

  useMountEffect(() => {
    searchInputRef.current?.focus();

    const editor = editorRef?.current;
    return () => editor?.commands.setSearch("", false);
  });

  useEffect(() => {
    if (allowReplace && search?.showReplace) {
      replaceInputRef.current?.focus();
    }
  }, [allowReplace, search?.showReplace]);

  if (!search) {
    return null;
  }

  const {
    query,
    currentMatchIndex,
    totalMatches,
    onNext,
    onPrev,
    caseSensitive,
    wholeWord,
    showReplace,
    replaceQuery,
    toggleWholeWord,
    toggleReplace,
    setReplaceQuery,
  } = search;

  const commands = editorRef?.current?.commands;

  const setQuery = (q: string) => {
    search.setQuery(q);
    commands?.setSearch(q, caseSensitive);
  };

  const toggleCaseSensitive = () => {
    search.toggleCaseSensitive();
    commands?.setSearch(query, !caseSensitive);
  };

  const close = () => {
    search.close();
    commands?.setSearch("", false);
  };

  const replaceCurrent = () => {
    if (!query || totalMatches === 0) return;
    commands?.replace({
      query,
      replacement: replaceQuery,
      caseSensitive,
      wholeWord,
      all: false,
      matchIndex: currentMatchIndex,
    });
  };

  const replaceAll = () => {
    if (!query) return;
    commands?.replace({
      query,
      replacement: replaceQuery,
      caseSensitive,
      wholeWord,
      all: true,
      matchIndex: 0,
    });
  };

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
      <div className="bg-muted flex h-7 items-center gap-1.5 rounded-lg px-2">
        <input
          ref={searchInputRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleSearchKeyDown}
          placeholder={t`Search...`}
          className="placeholder:text-muted-foreground h-full min-w-0 flex-1 bg-transparent text-xs focus:outline-hidden"
        />
        <div className="flex items-center gap-0.5">
          <ToggleButton
            active={caseSensitive}
            onClick={toggleCaseSensitive}
            tooltip={t`Match case`}
          >
            <TextAa className="size-3.5" />
          </ToggleButton>
          <ToggleButton
            active={wholeWord}
            onClick={toggleWholeWord}
            tooltip={t`Match whole word`}
          >
            <Textbox className="size-3.5" />
          </ToggleButton>
          {allowReplace && (
            <ToggleButton
              active={showReplace}
              onClick={toggleReplace}
              tooltip={
                <>
                  <span>
                    <Trans>Replace</Trans>
                  </span>
                  <Kbd className="animate-kbd-press">{primaryModifier} H</Kbd>
                </>
              }
            >
              <Swap className="size-3.5" />
            </ToggleButton>
          )}
        </div>
        <span className="text-muted-foreground text-[10px] whitespace-nowrap tabular-nums">
          {displayCount}
        </span>
        <div className="flex items-center">
          <IconButton
            onClick={onPrev}
            disabled={totalMatches === 0}
            tooltip={
              <>
                <span>
                  <Trans>Previous match</Trans>
                </span>
                <Kbd className="animate-kbd-press">⇧ ↵</Kbd>
              </>
            }
          >
            <CaretUp className="size-3.5" />
          </IconButton>
          <IconButton
            onClick={onNext}
            disabled={totalMatches === 0}
            tooltip={
              <>
                <span>
                  <Trans>Next match</Trans>
                </span>
                <Kbd className="animate-kbd-press">↵</Kbd>
              </>
            }
          >
            <CaretDown className="size-3.5" />
          </IconButton>
        </div>
        <IconButton
          onClick={close}
          tooltip={
            <>
              <span>
                <Trans>Close</Trans>
              </span>
              <Kbd className="animate-kbd-press">Esc</Kbd>
            </>
          }
        >
          <X className="size-3.5" />
        </IconButton>
      </div>

      {allowReplace && showReplace && (
        <div className="bg-muted flex h-7 items-center gap-1.5 rounded-lg px-2">
          <input
            ref={replaceInputRef}
            type="text"
            value={replaceQuery}
            onChange={(e) => setReplaceQuery(e.target.value)}
            onKeyDown={handleReplaceKeyDown}
            placeholder={t`Replace with...`}
            className="placeholder:text-muted-foreground h-full min-w-0 flex-1 bg-transparent text-xs focus:outline-hidden"
          />
          <div className="flex items-center gap-0.5">
            <IconButton
              onClick={replaceCurrent}
              tooltip={
                <>
                  <span>
                    <Trans>Replace</Trans>
                  </span>
                  <Kbd className="animate-kbd-press">↵</Kbd>
                </>
              }
            >
              <Swap className="size-3.5" />
            </IconButton>
            <IconButton
              onClick={replaceAll}
              tooltip={
                <>
                  <span>
                    <Trans>Replace all</Trans>
                  </span>
                  <Kbd className="animate-kbd-press">{primaryModifier} ↵</Kbd>
                </>
              }
            >
              <Repeat className="size-3.5" />
            </IconButton>
          </div>
        </div>
      )}
    </div>
  );
}
