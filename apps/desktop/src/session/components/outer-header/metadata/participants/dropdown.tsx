import { ArrowElbowDownLeft } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { type CSSProperties, useEffect, useRef } from "react";

import { colors, radii, shadows } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

type DropdownOption = {
  id: string;
  name: string;
  isNew?: boolean;
  email?: string;
  orgId?: string;
  jobTitle?: string;
};

export function ParticipantDropdown({
  floatingRef,
  floatingStyles,
  options,
  selectedIndex,
  onSelect,
  onHover,
}: {
  floatingRef: (node: HTMLDivElement | null) => void;
  floatingStyles: CSSProperties;
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
    <div
      ref={floatingRef}
      {...mergeStyleXProps(styles.root, undefined, floatingStyles)}
      onMouseDown={(event) => {
        event.preventDefault();
        event.stopPropagation();
      }}
    >
      <div ref={listRef} {...stylex.props(styles.list)}>
        {options.map((option, index) => (
          <button
            key={option.id}
            type="button"
            tabIndex={-1}
            {...stylex.props(
              styles.option,
              selectedIndex === index
                ? styles.selectedOption
                : styles.unselectedOption,
            )}
            onClick={() => onSelect(option)}
            onMouseEnter={() => onHover(index)}
          >
            <span {...stylex.props(styles.optionContent)}>
              {option.isNew ? (
                <span>
                  Add "
                  <span {...stylex.props(styles.mediumText)}>
                    {option.name}
                  </span>
                  "
                </span>
              ) : (
                <span {...stylex.props(styles.optionDetails)}>
                  <span {...stylex.props(styles.mediumText)}>
                    {option.name}
                  </span>
                  {option.jobTitle && (
                    <span {...stylex.props(styles.jobTitle)}>
                      {option.jobTitle}
                    </span>
                  )}
                </span>
              )}
              {selectedIndex === index && (
                <ArrowElbowDownLeft {...stylex.props(styles.enterIcon)} />
              )}
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}

const styles = stylex.create({
  enterIcon: {
    color: colors.mutedForeground,
    height: "0.75rem",
    width: "0.75rem",
  },
  jobTitle: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
  },
  list: {
    maxHeight: "12.5rem",
    overflow: "auto",
    paddingBlock: "0.25rem",
  },
  mediumText: {
    fontWeight: 500,
  },
  option: {
    fontSize: "0.875rem",
    paddingBlock: "0.375rem",
    paddingInline: "0.75rem",
    textAlign: "left",
    width: "100%",
  },
  optionContent: {
    alignItems: "center",
    display: "flex",
    justifyContent: "space-between",
    width: "100%",
  },
  optionDetails: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
  },
  root: {
    backgroundColor: colors.popover,
    borderColor: colors.border,
    borderRadius: radii.md,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: shadows.lg,
    overflow: "hidden",
  },
  selectedOption: {
    backgroundColor: colors.muted,
  },
  unselectedOption: {
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
  },
});
