import { Check, ChevronDown } from "lucide-react";
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
import { cn } from "@hypr/utils";

export interface SearchableSelectOption {
  value: string;
  label: string;
  detail?: string;
}

interface SearchableSelectProps {
  value: string;
  onChange: (value: string) => void;
  options: SearchableSelectOption[];
  placeholder?: string;
  searchPlaceholder?: string;
  emptyMessage?: string;
  className?: string;
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
  placeholder = "Select...",
  searchPlaceholder = "Search...",
  emptyMessage = "No results found.",
  className,
}: SearchableSelectProps) {
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
          className={cn([
            "justify-between bg-white font-normal shadow-none focus-visible:ring-0",
            "rounded-lg px-3",
            className,
          ])}
        >
          <span className="truncate">
            {selectedOption
              ? selectedOption.detail
                ? `${selectedOption.label} (${selectedOption.detail})`
                : selectedOption.label
              : placeholder}
          </span>
          <ChevronDown className="-mr-1 h-4 w-4 shrink-0 opacity-50" />
        </Button>
      </PopoverTrigger>
      <PopoverContent
        className="p-0"
        align="start"
        style={{ width: "var(--radix-popover-trigger-width)" }}
      >
        <Command filter={filterFunction}>
          <CommandInput
            placeholder={searchPlaceholder}
            value={query}
            onValueChange={setQuery}
          />
          <CommandEmpty>
            <div className="text-muted-foreground px-2 py-1.5 text-sm">
              {emptyMessage}
            </div>
          </CommandEmpty>
          <CommandList>
            <CommandGroup className="max-h-[250px] overflow-y-auto">
              {options.map((option) => (
                <CommandItem
                  key={option.value}
                  value={
                    option.detail
                      ? `${option.label} ${option.detail}`
                      : option.label
                  }
                  onSelect={() => handleSelect(option.value)}
                  className={cn([
                    "cursor-pointer",
                    "hover:bg-neutral-200! focus:bg-neutral-200! aria-selected:bg-transparent",
                  ])}
                >
                  <span className="flex-1 truncate">{option.label}</span>
                  {option.detail && (
                    <span className="shrink-0 font-mono text-[10px] text-neutral-400">
                      {option.detail}
                    </span>
                  )}
                  {value === option.value && (
                    <Check className="h-4 w-4 shrink-0" />
                  )}
                </CommandItem>
              ))}
            </CommandGroup>
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  );
}
