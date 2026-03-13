import {
  Check,
  ChevronDown,
  CirclePlus,
  Eye,
  EyeOff,
  RefreshCcw,
} from "lucide-react";
import { useCallback, useMemo, useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@hypr/ui/components/ui/command";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import type { ListModelsResult, ModelIgnoreReason } from "./list-common";

import { useModelMetadata } from "~/ai/hooks";

const filterFunction = (value: string, search: string) => {
  const v = value.toLocaleLowerCase();
  const s = search.toLocaleLowerCase();
  if (v.includes(s)) {
    return 1;
  }
  return 0;
};

const formatIgnoreReason = (reason: ModelIgnoreReason): string => {
  switch (reason) {
    case "common_keyword":
      return "Contains common ignore keyword";
    case "old_model":
      return "Old or deprecated model";
    case "date_snapshot":
      return "Date-specific snapshot";
    case "no_tool":
      return "No tool support";
    case "no_text_input":
      return "No text input support";
    case "no_completion":
      return "No completion support";
    case "not_llm":
      return "Not an LLM type";
    case "not_chat_model":
      return "Not a chat model";
    case "context_too_small":
      return "Context length too small";
  }
};

const getDisplayName = (providerId: string, model: string): string => {
  if (providerId === "hyprnote" && model === "Auto") {
    return "Pro (Cloud)";
  }
  return model;
};

export function ModelCombobox({
  providerId,
  value,
  onChange,
  listModels,
  disabled = false,
  placeholder = "Select a model",
  suffix,
  isConfigured = false,
}: {
  providerId: string;
  value: string;
  onChange: (value: string) => void;
  listModels?: () => Promise<ListModelsResult> | ListModelsResult;
  disabled?: boolean;
  placeholder?: string;
  suffix?: React.ReactNode;
  isConfigured?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [showIgnored, setShowIgnored] = useState(false);

  const {
    data: fetchedResult,
    isLoading: isLoadingModels,
    refetch,
    isFetching,
  } = useModelMetadata(providerId, listModels, { enabled: !disabled });

  const options: string[] = useMemo(
    () => fetchedResult?.models ?? [],
    [fetchedResult],
  );
  const ignoredOptions = useMemo(
    () => fetchedResult?.ignored ?? [],
    [fetchedResult],
  );
  const trimmedQuery = query.trim();
  const hasExactMatch = useMemo(
    () =>
      options.some(
        (option) =>
          option.toLocaleLowerCase() === trimmedQuery.toLocaleLowerCase(),
      ),
    [options, trimmedQuery],
  );
  const canSelectFreeform = trimmedQuery.length > 0 && !hasExactMatch;
  const hasIgnoredOptions = ignoredOptions.length > 0;

  const handleSelect = useCallback(
    (option: string) => {
      onChange(option);
      setOpen(false);
      setQuery("");
    },
    [onChange],
  );

  const toggleShowIgnored = useCallback(
    () => setShowIgnored((prev) => !prev),
    [],
  );

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          type="button"
          variant="outline"
          role="combobox"
          disabled={disabled || isLoadingModels}
          aria-expanded={open}
          className={cn([
            "w-full justify-between bg-white font-normal shadow-none focus-visible:ring-0",
            "rounded-md px-3",
          ])}
        >
          <span className="flex w-full min-w-0 items-center justify-between gap-2">
            {value && value.length > 0 ? (
              <span className="truncate">
                {getDisplayName(providerId, value)}
              </span>
            ) : (
              <span className="text-muted-foreground truncate">
                {isLoadingModels ? "Loading models..." : placeholder}
              </span>
            )}
            {suffix}
          </span>
          {isConfigured ? (
            <Check className="-mr-1 h-4 w-4 shrink-0 text-green-600" />
          ) : (
            <ChevronDown className="-mr-1 h-4 w-4 shrink-0 opacity-50" />
          )}
        </Button>
      </PopoverTrigger>
      <PopoverContent
        className="p-0"
        style={{ width: "var(--radix-popover-trigger-width)" }}
      >
        <Command filter={filterFunction}>
          <CommandInput
            placeholder="Search or create new"
            value={query}
            onValueChange={(value: string) => setQuery(value)}
            onKeyDown={(event: React.KeyboardEvent<HTMLInputElement>) => {
              if (event.key === "Enter") {
                event.preventDefault();
              }
            }}
          />
          <CommandEmpty>
            <div className="text-muted-foreground px-2 py-1.5 text-sm">
              {trimmedQuery.length > 0 ? (
                <p>No results found.</p>
              ) : hasIgnoredOptions ? (
                <p>No models ready to use.</p>
              ) : (
                <p>No models available.</p>
              )}
            </div>
          </CommandEmpty>

          <CommandList>
            <CommandGroup className="overflow-y-auto">
              {options.map((option) => (
                <CommandItem
                  key={option}
                  tabIndex={0}
                  value={option}
                  onSelect={() => {
                    handleSelect(option);
                  }}
                  onKeyDown={(event: React.KeyboardEvent<HTMLDivElement>) => {
                    if (event.key === "Enter") {
                      event.stopPropagation();
                      handleSelect(option);
                    }
                  }}
                  className={cn([
                    "cursor-pointer",
                    "hover:bg-neutral-200! focus:bg-neutral-200! aria-selected:bg-transparent",
                  ])}
                >
                  <span className="truncate">
                    {getDisplayName(providerId, option)}
                  </span>
                </CommandItem>
              ))}

              {showIgnored &&
                ignoredOptions.map((option) => (
                  <CommandItem
                    key={`ignored-${option.id}`}
                    tabIndex={0}
                    value={option.id}
                    onSelect={() => {
                      handleSelect(option.id);
                    }}
                    onKeyDown={(event: React.KeyboardEvent<HTMLDivElement>) => {
                      if (event.key === "Enter") {
                        event.stopPropagation();
                        handleSelect(option.id);
                      }
                    }}
                    className={cn([
                      "cursor-pointer opacity-50",
                      "hover:bg-neutral-200! focus:bg-neutral-200! aria-selected:bg-transparent",
                    ])}
                  >
                    <Tooltip delayDuration={10}>
                      <TooltipTrigger asChild>
                        <span className="w-full truncate">{option.id}</span>
                      </TooltipTrigger>
                      <TooltipContent side="right" className="text-xs">
                        <div className="flex flex-col gap-0.5">
                          {option.reasons.map((reason) => (
                            <div key={reason}>
                              • {formatIgnoreReason(reason)}
                            </div>
                          ))}
                        </div>
                      </TooltipContent>
                    </Tooltip>
                  </CommandItem>
                ))}

              {canSelectFreeform && (
                <CommandItem
                  key={`freeform-${trimmedQuery}`}
                  tabIndex={0}
                  value={trimmedQuery}
                  onSelect={() => {
                    handleSelect(trimmedQuery);
                  }}
                  onKeyDown={(event: React.KeyboardEvent<HTMLDivElement>) => {
                    if (event.key === "Enter") {
                      event.stopPropagation();
                      handleSelect(trimmedQuery);
                    }
                  }}
                  className={cn([
                    "cursor-pointer",
                    "hover:bg-neutral-200! focus:bg-neutral-200! aria-selected:bg-transparent",
                  ])}
                >
                  <CirclePlus className="mr-2 h-4 w-4" />
                  <span className="truncate">Select "{trimmedQuery}"</span>
                </CommandItem>
              )}
            </CommandGroup>
          </CommandList>

          <div className="text-muted-foreground flex items-center justify-between border-t px-2 py-1.5 text-xs">
            <button
              type="button"
              onClick={toggleShowIgnored}
              className="hover:text-foreground mr-1 flex items-center gap-1 text-xs transition-colors"
            >
              {showIgnored ? (
                <EyeOff className="h-3 w-3" />
              ) : (
                <Eye className="h-3 w-3" />
              )}
            </button>

            {hasIgnoredOptions && (
              <span>
                {showIgnored
                  ? `Showing total of ${options.length} models.`
                  : `${ignoredOptions.length} items ignored.`}
              </span>
            )}

            <button
              type="button"
              onClick={() => refetch()}
              disabled={isFetching}
              className="hover:text-foreground ml-auto flex items-center gap-1 text-xs transition-colors disabled:opacity-50"
            >
              <RefreshCcw
                className={cn(["h-3 w-3", isFetching && "animate-spin"])}
              />
            </button>
          </div>
        </Command>
      </PopoverContent>
    </Popover>
  );
}
