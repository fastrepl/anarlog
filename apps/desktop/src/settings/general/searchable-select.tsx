import { useLingui } from "@lingui/react/macro";
import { CaretDown, Check } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useCallback, useMemo, useState } from "react";

import { colors, fonts } from "@anlg/design-system/tokens.stylex";
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
import type { StyleXProps } from "@anlg/ui/lib/stylex";

export interface SearchableSelectOption {
  value: string;
  label: string;
  detail?: string;
}

interface SearchableSelectProps extends StyleXProps {
  value: string;
  onChange: (value: string) => void;
  options: SearchableSelectOption[];
  placeholder?: string;
  searchPlaceholder?: string;
  emptyMessage?: string;
  dropdownSx?: StyleXProps["sx"];
  "aria-labelledby"?: string;
  "aria-describedby"?: string;
}

const filterFunction = (value: string, search: string) => {
  const v = value.toLocaleLowerCase();
  const s = search.toLocaleLowerCase();
  if (v.includes(s)) {
    return 1;
  }
  return 0;
};

export function SearchableSelect({
  value,
  onChange,
  options,
  placeholder,
  searchPlaceholder,
  emptyMessage,
  sx,
  dropdownSx,
  "aria-labelledby": ariaLabelledBy,
  "aria-describedby": ariaDescribedBy,
}: SearchableSelectProps) {
  const { t } = useLingui();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");

  const selectedOption = useMemo(
    () => options.find((opt) => opt.value === value),
    [options, value],
  );

  const handleSelect = useCallback(
    (optionValue: string) => {
      onChange(optionValue);
      setOpen(false);
      setQuery("");
    },
    [onChange],
  );

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          type="button"
          variant="outline"
          role="combobox"
          aria-expanded={open}
          aria-labelledby={ariaLabelledBy}
          aria-describedby={ariaDescribedBy}
          sx={[styles.trigger, sx]}
        >
          <span {...stylex.props(styles.selectedValue)}>
            {selectedOption
              ? selectedOption.detail
                ? `${selectedOption.label} (${selectedOption.detail})`
                : selectedOption.label
              : (placeholder ?? t`Select...`)}
          </span>
          <CaretDown {...stylex.props(styles.caret)} />
        </Button>
      </PopoverTrigger>
      <PopoverContent
        variant="app"
        align="start"
        sx={[styles.popover, dropdownSx]}
      >
        <AppFloatingPanel sx={styles.floatingPanel}>
          <Command filter={filterFunction} sx={styles.command}>
            <CommandInput
              placeholder={searchPlaceholder ?? t`Search...`}
              value={query}
              onValueChange={setQuery}
            />
            <CommandEmpty>
              <div {...stylex.props(styles.empty)}>
                {emptyMessage ?? t`No results found.`}
              </div>
            </CommandEmpty>
            <CommandList>
              <CommandGroup sx={styles.group}>
                {options.map((option) => (
                  <CommandItem
                    key={option.value}
                    value={
                      option.detail
                        ? `${option.label} ${option.detail}`
                        : option.label
                    }
                    onSelect={() => handleSelect(option.value)}
                    sx={styles.item}
                  >
                    <span {...stylex.props(styles.optionLabel)}>
                      {option.label}
                    </span>
                    {option.detail && (
                      <span {...stylex.props(styles.optionDetail)}>
                        {option.detail}
                      </span>
                    )}
                    {value === option.value && (
                      <Check {...stylex.props(styles.check)} />
                    )}
                  </CommandItem>
                ))}
              </CommandGroup>
            </CommandList>
          </Command>
        </AppFloatingPanel>
      </PopoverContent>
    </Popover>
  );
}

const styles = stylex.create({
  caret: {
    flexShrink: 0,
    height: "1rem",
    marginRight: "-0.25rem",
    opacity: 0.5,
    width: "1rem",
  },
  check: {
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
  command: {
    backgroundColor: "transparent",
    borderWidth: 0,
    borderRadius: "inherit",
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
  group: {
    maxHeight: "250px",
    overflowY: "auto",
  },
  item: {
    backgroundColor: {
      default: "transparent",
      ":focus": colors.accent,
      ":hover": colors.accent,
    },
    cursor: "pointer",
  },
  optionDetail: {
    color: colors.mutedForeground,
    flexShrink: 0,
    fontFamily: fonts.mono,
    fontSize: "10px",
  },
  optionLabel: {
    flex: "1",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  popover: {
    width: "var(--radix-popover-trigger-width)",
  },
  selectedValue: {
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  trigger: {
    backgroundColor: colors.card,
    borderRadius: "9999px",
    boxShadow: {
      default: "none",
      ":focus-visible": "none",
    },
    fontWeight: 400,
    justifyContent: "space-between",
    paddingInline: "0.75rem",
  },
});
