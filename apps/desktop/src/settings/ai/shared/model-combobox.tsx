import { Trans, useLingui } from "@lingui/react/macro";
import {
  ArrowsCounterClockwise,
  CaretDown,
  Check,
  Eye,
  EyeSlash,
  PlusCircle,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useCallback, useMemo, useState } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@anlg/ui/components/ui/command";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@anlg/ui/components/ui/popover";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@anlg/ui/components/ui/tooltip";

import type { ListModelsResult, ModelIgnoreReason } from "./list-common";
import { displayLlmModelId } from "./model-display";

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
  return displayLlmModelId(providerId, model);
};

export function ModelCombobox({
  providerId,
  value,
  onChange,
  listModels,
  disabled = false,
  placeholder,
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
  const { t } = useLingui();
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
  const isSelectedDeprecated = ignoredOptions.some(
    (option) => option.id === value && option.reasons.includes("old_model"),
  );

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
          sx={styles.trigger}
        >
          <span {...stylex.props(styles.triggerContent)}>
            {value && value.length > 0 ? (
              <span {...stylex.props(styles.selectedModel)}>
                <span
                  {...stylex.props(
                    styles.truncated,
                    isSelectedDeprecated && styles.deprecatedModel,
                  )}
                >
                  {getDisplayName(providerId, value)}
                </span>
                {isSelectedDeprecated ? <DeprecatedBadge /> : null}
              </span>
            ) : (
              <span {...stylex.props(styles.placeholder)}>
                {isLoadingModels
                  ? t`Loading models...`
                  : (placeholder ?? t`Select a model`)}
              </span>
            )}
            {suffix}
          </span>
          {isConfigured ? (
            <Check {...stylex.props(styles.configuredIcon)} />
          ) : (
            <CaretDown {...stylex.props(styles.caret)} />
          )}
        </Button>
      </PopoverTrigger>
      <PopoverContent variant="app" sx={styles.popover}>
        <AppFloatingPanel sx={styles.floatingPanel}>
          <Command filter={filterFunction} sx={styles.command}>
            <CommandInput
              placeholder={t`Search or create new`}
              value={query}
              onValueChange={(value: string) => setQuery(value)}
              onKeyDown={(event: React.KeyboardEvent<HTMLInputElement>) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                }
              }}
            />
            <CommandEmpty>
              <div {...stylex.props(styles.empty)}>
                {trimmedQuery.length > 0 ? (
                  <p>
                    <Trans>No results found.</Trans>
                  </p>
                ) : hasIgnoredOptions ? (
                  <p>
                    <Trans>No models ready to use.</Trans>
                  </p>
                ) : (
                  <p>
                    <Trans>No models available.</Trans>
                  </p>
                )}
              </div>
            </CommandEmpty>

            <CommandList>
              <CommandGroup sx={styles.group}>
                {options.map((option) => (
                  <CommandItem
                    key={option}
                    tabIndex={0}
                    value={`${option} ${getDisplayName(providerId, option)}`}
                    onSelect={() => {
                      handleSelect(option);
                    }}
                    onKeyDown={(event: React.KeyboardEvent<HTMLDivElement>) => {
                      if (event.key === "Enter") {
                        event.stopPropagation();
                        handleSelect(option);
                      }
                    }}
                    sx={styles.item}
                  >
                    <span {...stylex.props(styles.truncated)}>
                      {getDisplayName(providerId, option)}
                    </span>
                  </CommandItem>
                ))}

                {showIgnored &&
                  ignoredOptions.map((option) => (
                    <CommandItem
                      key={`ignored-${option.id}`}
                      tabIndex={0}
                      value={`${option.id} ${getDisplayName(providerId, option.id)}`}
                      onSelect={() => {
                        handleSelect(option.id);
                      }}
                      onKeyDown={(
                        event: React.KeyboardEvent<HTMLDivElement>,
                      ) => {
                        if (event.key === "Enter") {
                          event.stopPropagation();
                          handleSelect(option.id);
                        }
                      }}
                      sx={[styles.item, styles.ignoredItem]}
                    >
                      <Tooltip delayDuration={10}>
                        <TooltipTrigger asChild>
                          <span {...stylex.props(styles.ignoredContent)}>
                            <span {...stylex.props(styles.truncated)}>
                              {getDisplayName(providerId, option.id)}
                            </span>
                            {option.reasons.includes("old_model") ? (
                              <DeprecatedBadge />
                            ) : null}
                          </span>
                        </TooltipTrigger>
                        <TooltipContent side="right" sx={styles.tooltip}>
                          <div {...stylex.props(styles.reasons)}>
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
                    sx={styles.item}
                  >
                    <PlusCircle {...stylex.props(styles.plusIcon)} />
                    <span {...stylex.props(styles.truncated)}>
                      Select "{trimmedQuery}"
                    </span>
                  </CommandItem>
                )}
              </CommandGroup>
            </CommandList>

            <div {...stylex.props(styles.footer)}>
              <button
                type="button"
                onClick={toggleShowIgnored}
                {...stylex.props(styles.footerButton, styles.ignoredToggle)}
              >
                {showIgnored ? (
                  <EyeSlash {...stylex.props(styles.footerIcon)} />
                ) : (
                  <Eye {...stylex.props(styles.footerIcon)} />
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
                {...stylex.props(styles.footerButton, styles.refreshButton)}
              >
                <ArrowsCounterClockwise
                  {...stylex.props(
                    styles.footerIcon,
                    isFetching && styles.spinning,
                  )}
                />
              </button>
            </div>
          </Command>
        </AppFloatingPanel>
      </PopoverContent>
    </Popover>
  );
}

function DeprecatedBadge() {
  return (
    <span {...stylex.props(styles.deprecatedBadge)}>
      <Trans>Deprecated</Trans>
    </span>
  );
}

const spin = stylex.keyframes({
  to: { transform: "rotate(360deg)" },
});

const styles = stylex.create({
  caret: {
    flexShrink: 0,
    height: "1rem",
    marginRight: "-0.25rem",
    opacity: 0.5,
    width: "1rem",
  },
  command: {
    backgroundColor: "transparent",
    borderRadius: "inherit",
    borderWidth: 0,
  },
  configuredIcon: {
    color: "rgb(22 163 74)",
    flexShrink: 0,
    height: "1rem",
    marginRight: "-0.25rem",
    width: "1rem",
  },
  deprecatedBadge: {
    backgroundColor: "rgb(255 251 235)",
    borderRadius: radii.md,
    color: "rgb(146 64 14)",
    flexShrink: 0,
    fontSize: "11px",
    fontWeight: 500,
    paddingBlock: "0.125rem",
    paddingInline: "0.375rem",
  },
  deprecatedModel: {
    color: colors.mutedForeground,
  },
  empty: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    paddingBlock: "0.375rem",
    paddingInline: "0.5rem",
  },
  floatingPanel: {
    overflow: "hidden",
  },
  footer: {
    alignItems: "center",
    borderTopColor: colors.border,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    color: colors.mutedForeground,
    display: "flex",
    fontSize: "0.75rem",
    justifyContent: "space-between",
    lineHeight: "1rem",
    paddingBlock: "0.375rem",
    paddingInline: "0.5rem",
  },
  footerButton: {
    alignItems: "center",
    color: {
      default: "inherit",
      ":hover": colors.foreground,
    },
    display: "flex",
    fontSize: "0.75rem",
    gap: "0.25rem",
    lineHeight: "1rem",
    opacity: {
      default: 1,
      ":disabled": 0.5,
    },
    transitionDuration: "150ms",
    transitionProperty: "color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  footerIcon: {
    height: "0.75rem",
    width: "0.75rem",
  },
  group: {
    overflowY: "auto",
  },
  ignoredContent: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    minWidth: 0,
    width: "100%",
  },
  ignoredItem: {
    opacity: 0.5,
  },
  ignoredToggle: {
    marginRight: "0.25rem",
  },
  item: {
    backgroundColor: {
      default: "transparent",
      ":focus": colors.accent,
      ":hover": colors.accent,
    },
    cursor: "pointer",
  },
  placeholder: {
    color: colors.mutedForeground,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  plusIcon: {
    height: "1rem",
    marginRight: "0.5rem",
    width: "1rem",
  },
  popover: {
    width: "var(--radix-popover-trigger-width)",
  },
  reasons: {
    display: "flex",
    flexDirection: "column",
    gap: "0.125rem",
  },
  refreshButton: {
    marginLeft: "auto",
  },
  selectedModel: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    minWidth: 0,
  },
  spinning: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
  },
  tooltip: {
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  trigger: {
    backgroundColor: colors.card,
    borderRadius: radii.full,
    boxShadow: {
      default: "none",
      ":focus-visible": "none",
    },
    fontWeight: 400,
    justifyContent: "space-between",
    paddingInline: "0.75rem",
    width: "100%",
  },
  triggerContent: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    justifyContent: "space-between",
    minWidth: 0,
    width: "100%",
  },
  truncated: {
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
});
