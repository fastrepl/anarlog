import { CornerDownLeft } from "lucide-react";
import { useEffect, useRef } from "react";

import { cn } from "@hypr/utils";

type DropdownOption = {
  id: string;
  name: string;
  isNew?: boolean;
  email?: string;
  orgId?: string;
  jobTitle?: string;
};

export function ParticipantDropdown({
  options,
  selectedIndex,
  onSelect,
  onHover,
}: {
  options: DropdownOption[];
  selectedIndex: number;
  onSelect: (option: DropdownOption) => void;
  onHover: (index: number) => void;
}) {
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const list = listRef.current;
    if (!list) return;

    const selectedElement = list.children[selectedIndex] as HTMLElement;
    if (selectedElement) {
      selectedElement.scrollIntoView({ block: "nearest" });
    }
  }, [selectedIndex, options]);

  if (options.length === 0) {
    return null;
  }

  return (
    <div className="bg-popover absolute z-50 mt-1 w-full overflow-hidden rounded-md border shadow-md">
      <div ref={listRef} className="max-h-50 overflow-auto py-1">
        {options.map((option, index) => (
          <button
            key={option.id}
            type="button"
            tabIndex={-1}
            className={cn([
              "w-full px-3 py-1.5 text-left text-sm",
              selectedIndex === index
                ? "bg-neutral-100"
                : "hover:bg-neutral-50",
            ])}
            onClick={() => onSelect(option)}
            onMouseEnter={() => onHover(index)}
          >
            <span className="flex w-full items-center justify-between">
              {option.isNew ? (
                <span>
                  Add "<span className="font-medium">{option.name}</span>"
                </span>
              ) : (
                <span className="flex items-center gap-2">
                  <span className="font-medium">{option.name}</span>
                  {option.jobTitle && (
                    <span className="text-muted-foreground text-xs">
                      {option.jobTitle}
                    </span>
                  )}
                </span>
              )}
              {selectedIndex === index && (
                <CornerDownLeft className="text-muted-foreground size-3" />
              )}
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}
