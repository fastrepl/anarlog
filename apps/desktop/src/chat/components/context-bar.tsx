import { CaretUp, XCircle } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMemo, useState } from "react";

import { colors, shadows } from "@anlg/design-system/tokens.stylex";

import { type ContextChipProps, renderChip } from "~/chat/context/registry";
import type { DisplayEntity } from "~/chat/context/use-chat-context-pipeline";
import { useTabs } from "~/store/zustand/tabs";

const COLLAPSED_CONTEXT_CHIP_LIMIT = 4;

function ContextChip({
  chip,
  onRemove,
  pending,
}: {
  chip: ContextChipProps;
  onRemove?: (key: string) => void;
  pending?: boolean;
}) {
  const Icon = chip.icon;
  const openNew = useTabs((state) => state.openNew);
  const isClickable = chip.tab != null;

  const handleClick = () => {
    if (!chip.tab) {
      return;
    }

    openNew(chip.tab);
  };

  return (
    <span
      data-chat-context-chip
      onClick={handleClick}
      {...stylex.props([
        styles.chip,
        pending ? styles.pendingChip : styles.readyChip,
        isClickable ? styles.clickableChip : styles.staticChip,
      ])}
    >
      <span {...stylex.props(styles.iconSlot)}>
        <Icon
          {...stylex.props([
            styles.contextIcon,
            chip.removable && onRemove && styles.hideContextIconOnHover,
          ])}
        />
        {chip.removable && onRemove && (
          <button
            type="button"
            aria-label={`Remove ${chip.label}`}
            onClick={(e) => {
              e.stopPropagation();
              onRemove(chip.key);
            }}
            {...stylex.props(styles.removeButton)}
          >
            <XCircle {...stylex.props(styles.smallIcon)} />
          </button>
        )}
      </span>
      <span {...stylex.props(styles.truncate)}>{chip.label}</span>
    </span>
  );
}

function ChipList({
  chips,
  onRemove,
}: {
  chips: Array<{ chip: ContextChipProps; pending: boolean }>;
  onRemove?: (key: string) => void;
}) {
  const [isExpanded, setIsExpanded] = useState(false);
  const hiddenCount = Math.max(0, chips.length - COLLAPSED_CONTEXT_CHIP_LIMIT);
  const visibleChips = isExpanded
    ? chips
    : chips.slice(0, COLLAPSED_CONTEXT_CHIP_LIMIT);
  const canExpand = hiddenCount > 0;

  return (
    <div {...stylex.props(styles.row)}>
      <div
        data-chat-context-chip-list
        {...stylex.props([
          styles.chipList,
          !isExpanded && styles.collapsedChipList,
        ])}
      >
        <div
          data-chat-context-chip-strip
          {...stylex.props([
            styles.chipStrip,
            isExpanded ? styles.expandedChipStrip : styles.collapsedChipStrip,
          ])}
        >
          {visibleChips.map(({ chip, pending }) => (
            <ContextChip
              key={chip.key}
              chip={chip}
              onRemove={onRemove}
              pending={pending}
            />
          ))}
        </div>
      </div>

      {canExpand && (
        <button
          type="button"
          data-chat-context-overflow-chip
          aria-expanded={isExpanded}
          aria-label={isExpanded ? "Collapse context chips" : undefined}
          onClick={() => setIsExpanded((value) => !value)}
          {...stylex.props(styles.overflowButton)}
        >
          {isExpanded ? (
            <CaretUp aria-hidden="true" {...stylex.props(styles.caret)} />
          ) : (
            `+${hiddenCount} more`
          )}
        </button>
      )}
    </div>
  );
}

export function ContextBar({
  entities,
  onRemoveEntity,
}: {
  entities: DisplayEntity[];
  onRemoveEntity?: (key: string) => void;
}) {
  const chips = useMemo(
    () =>
      entities
        .map((entity) => ({
          chip: renderChip(entity),
          pending: entity.pending,
        }))
        .filter(
          (c): c is { chip: ContextChipProps; pending: boolean } =>
            c.chip !== null,
        ),
    [entities],
  );

  if (chips.length === 0) {
    return null;
  }

  return (
    <div data-chat-context-bar {...stylex.props(styles.bar)}>
      <ChipList chips={chips} onRemove={onRemoveEntity} />
    </div>
  );
}

const styles = stylex.create({
  chip: {
    alignItems: "center",
    borderColor: `color-mix(in oklab, ${colors.border} 60%, transparent)`,
    borderRadius: "10px",
    borderStyle: "solid",
    borderWidth: "1px",
    color: colors.mutedForeground,
    display: "inline-flex",
    flexShrink: 0,
    fontSize: "0.75rem",
    gap: "0.375rem",
    height: "1.75rem",
    lineHeight: "1rem",
    maxWidth: "14rem",
    minWidth: 0,
    paddingInline: "0.625rem",
    boxShadow: shadows.sm,
  },
  pendingChip: {
    backgroundColor: `color-mix(in oklab, ${colors.card} 60%, transparent)`,
  },
  readyChip: {
    backgroundColor: `color-mix(in oklab, ${colors.card} 90%, transparent)`,
  },
  clickableChip: {
    backgroundColor: {
      default: null,
      ":hover": `color-mix(in oklab, ${colors.accent} 20%, transparent)`,
    },
    cursor: "pointer",
  },
  staticChip: {
    cursor: "default",
  },
  iconSlot: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    height: "1rem",
    justifyContent: "center",
    position: "relative",
    width: "1rem",
  },
  contextIcon: {
    color: colors.mutedForeground,
    flexShrink: 0,
    height: "0.875rem",
    transitionDuration: "150ms",
    transitionProperty: "opacity",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "0.875rem",
  },
  hideContextIconOnHover: {
    opacity: {
      default: 1,
      ":is([data-chat-context-chip]:hover *)": 0,
    },
  },
  removeButton: {
    alignItems: "center",
    bottom: 0,
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    display: "flex",
    justifyContent: "center",
    left: 0,
    opacity: {
      default: 0,
      ":focus-visible": 1,
      ":is([data-chat-context-chip]:hover *)": 1,
    },
    pointerEvents: {
      default: "none",
      ":focus-visible": "auto",
      ":is([data-chat-context-chip]:hover *)": "auto",
    },
    position: "absolute",
    right: 0,
    top: 0,
    transitionDuration: "150ms",
    transitionProperty: "opacity",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  smallIcon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  truncate: {
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  row: {
    alignItems: "center",
    display: "flex",
    gap: "0.375rem",
    justifyContent: "center",
    minWidth: 0,
    width: "100%",
  },
  chipList: {
    minWidth: 0,
  },
  collapsedChipList: {
    overflowX: "auto",
    overscrollBehaviorX: "contain",
  },
  chipStrip: {
    alignItems: "center",
    display: "flex",
    gap: "0.375rem",
    justifyContent: "center",
  },
  expandedChipStrip: {
    flexWrap: "wrap",
    width: "100%",
  },
  collapsedChipStrip: {
    minWidth: "100%",
    width: "max-content",
  },
  overflowButton: {
    alignItems: "center",
    backgroundColor: {
      default: `color-mix(in oklab, ${colors.card} 70%, transparent)`,
      ":hover": `color-mix(in oklab, ${colors.accent} 20%, transparent)`,
    },
    borderColor: `color-mix(in oklab, ${colors.border} 60%, transparent)`,
    borderRadius: "10px",
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: shadows.sm,
    color: {
      default: colors.mutedForeground,
      ":hover": colors.mutedForeground,
    },
    display: "inline-flex",
    flexShrink: 0,
    fontSize: "0.75rem",
    gap: "0.125rem",
    height: "1.75rem",
    lineHeight: "1rem",
    paddingInline: "0.375rem",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color, border-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  caret: {
    height: "0.75rem",
    width: "0.75rem",
  },
  bar: {
    flexShrink: 0,
    paddingBottom: "0.375rem",
    paddingInline: "0.75rem",
  },
});
