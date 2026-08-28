import { Trans, useLingui } from "@lingui/react/macro";
import { DotsSixVertical, DotsThree, Plus } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { Reorder, useDragControls } from "motion/react";
import { useCallback, useEffect, useRef, useState } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import type { TemplateSection } from "@anlg/store";
import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { Input } from "@anlg/ui/components/ui/input";

type SectionDraft = TemplateSection & { key: string };

function useEditableSections({
  disabled,
  initialItems,
  onChange,
}: {
  disabled: boolean;
  initialItems: TemplateSection[];
  onChange: (items: TemplateSection[]) => void;
}) {
  const [drafts, setDrafts] = useState<SectionDraft[]>(() =>
    initialItems.map((s) => ({ ...s, key: crypto.randomUUID() })),
  );

  useEffect(() => {
    setDrafts((prev) => {
      const changed =
        prev.length !== initialItems.length ||
        prev.some(
          (d, i) =>
            d.title !== initialItems[i]?.title ||
            d.description !== initialItems[i]?.description,
        );
      if (!changed) return prev;
      return initialItems.map((s, i) => ({
        ...s,
        key: prev[i]?.key ?? crypto.randomUUID(),
      }));
    });
  }, [initialItems]);

  const pendingCommit = useRef<TemplateSection[] | null>(null);

  useEffect(() => {
    if (pendingCommit.current) {
      const value = pendingCommit.current;
      pendingCommit.current = null;
      onChange(value);
    }
  });

  const commit = useCallback(
    (next: SectionDraft[] | ((prev: SectionDraft[]) => SectionDraft[])) => {
      setDrafts((prev) => {
        const resolved = typeof next === "function" ? next(prev) : next;
        pendingCommit.current = resolved.map(({ title, description }) => ({
          title,
          description,
        }));
        return resolved;
      });
    },
    [],
  );

  return {
    drafts,
    addSection: useCallback(
      () =>
        commit((prev) => [
          ...prev,
          { title: "", description: "", key: crypto.randomUUID() },
        ]),
      [commit],
    ),
    changeSection: useCallback(
      (draft: SectionDraft) =>
        commit((prev) => prev.map((s) => (s.key === draft.key ? draft : s))),
      [commit],
    ),
    deleteSection: useCallback(
      (key: string) => commit((prev) => prev.filter((s) => s.key !== key)),
      [commit],
    ),
    insertSectionAt: useCallback(
      (index: number) =>
        commit((prev) => {
          const next = [...prev];
          next.splice(index, 0, {
            title: "",
            description: "",
            key: crypto.randomUUID(),
          });
          return next;
        }),
      [commit],
    ),
    moveSection: useCallback(
      (key: string, direction: -1 | 1) =>
        commit((prev) => {
          const i = prev.findIndex((s) => s.key === key);
          const j = i + direction;
          if (i < 0 || j < 0 || j >= prev.length) return prev;
          const next = [...prev];
          const [s] = next.splice(i, 1);
          next.splice(j, 0, s);
          return next;
        }),
      [commit],
    ),
    reorderSections: useCallback(
      (next: SectionDraft[]) => {
        if (!disabled) commit(next);
      },
      [commit, disabled],
    ),
  };
}

export function SectionsList({
  disabled,
  items,
  onChange,
}: {
  disabled: boolean;
  items: TemplateSection[];
  onChange: (items: TemplateSection[]) => void;
}) {
  const controls = useDragControls();
  const {
    drafts,
    addSection,
    changeSection,
    deleteSection,
    insertSectionAt,
    moveSection,
    reorderSections,
  } = useEditableSections({
    disabled,
    initialItems: items,
    onChange,
  });

  return (
    <div {...stylex.props(styles.root)}>
      <Reorder.Group values={drafts} onReorder={reorderSections}>
        <div {...stylex.props(styles.list)}>
          {drafts.map((draft, index) => (
            <Reorder.Item key={draft.key} value={draft}>
              <SectionItem
                disabled={disabled}
                index={index}
                total={drafts.length}
                item={draft}
                onChange={changeSection}
                onDelete={deleteSection}
                onInsertAbove={insertSectionAt}
                onInsertBelow={insertSectionAt}
                onMove={moveSection}
                dragControls={controls}
              />
            </Reorder.Item>
          ))}
        </div>
      </Reorder.Group>

      {!disabled && (
        <Button
          variant="outline"
          size="sm"
          sx={styles.addButton}
          onClick={addSection}
          disabled={disabled}
        >
          <Plus {...stylex.props(styles.addIcon)} />
          Add Section
        </Button>
      )}
    </div>
  );
}

function SectionItem({
  disabled,
  index,
  total,
  item,
  onChange,
  onDelete,
  onInsertAbove,
  onInsertBelow,
  onMove,
  dragControls,
}: {
  disabled: boolean;
  index: number;
  total: number;
  item: SectionDraft;
  onChange: (item: SectionDraft) => void;
  onDelete: (key: string) => void;
  onInsertAbove: (index: number) => void;
  onInsertBelow: (index: number) => void;
  onMove: (key: string, direction: -1 | 1) => void;
  dragControls: ReturnType<typeof useDragControls>;
}) {
  const { t } = useLingui();
  const [isFocused, setIsFocused] = useState(false);

  return (
    <div data-section-item {...stylex.props(styles.item)}>
      {!disabled && (
        <button
          type="button"
          {...stylex.props(styles.dragHandle)}
          onPointerDown={(event) => dragControls.start(event)}
          disabled={disabled}
        >
          <DotsSixVertical {...stylex.props(styles.dragIcon)} />
        </button>
      )}

      {!disabled && (
        <div {...stylex.props(styles.itemActions)}>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                type="button"
                size="icon"
                variant="ghost"
                sx={styles.actionButton}
                aria-label={t`Section actions`}
              >
                <DotsThree {...stylex.props(styles.icon)} />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent variant="app" align="end">
              <AppFloatingPanel sx={styles.menuPanel}>
                <DropdownMenuItem
                  onClick={() => onInsertAbove(index)}
                  sx={styles.menuItem}
                >
                  <Trans>Insert above</Trans>
                </DropdownMenuItem>
                <DropdownMenuItem
                  onClick={() => onInsertBelow(index + 1)}
                  sx={styles.menuItem}
                >
                  <Trans>Insert below</Trans>
                </DropdownMenuItem>
                <DropdownMenuItem
                  onClick={() => onMove(item.key, -1)}
                  disabled={index === 0}
                  sx={styles.menuItem}
                >
                  <Trans>Move up</Trans>
                </DropdownMenuItem>
                <DropdownMenuItem
                  onClick={() => onMove(item.key, 1)}
                  disabled={index === total - 1}
                  sx={styles.menuItem}
                >
                  <Trans>Move down</Trans>
                </DropdownMenuItem>
                <DropdownMenuItem
                  onClick={() => onDelete(item.key)}
                  sx={[styles.menuItem, styles.deleteItem]}
                >
                  <Trans>Delete</Trans>
                </DropdownMenuItem>
              </AppFloatingPanel>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      )}

      <div {...stylex.props(styles.fields)}>
        <Input
          disabled={disabled}
          value={item.title}
          onChange={(e) => onChange({ ...item, title: e.target.value })}
          placeholder={t`Untitled`}
          sx={styles.titleInput}
        />

        <textarea
          disabled={disabled}
          value={item.description}
          onChange={(e) => onChange({ ...item, description: e.target.value })}
          placeholder={t`Template content with Jinja2: {{ variable }}, {% if condition %}`}
          onFocus={() => setIsFocused(true)}
          onBlur={() => setIsFocused(false)}
          {...stylex.props([
            styles.descriptionInput,
            disabled
              ? styles.disabledDescription
              : isFocused
                ? styles.focusedDescription
                : styles.idleDescription,
          ])}
        />
      </div>
    </div>
  );
}

const styles = stylex.create({
  actionButton: {
    color: colors.mutedForeground,
    height: "1.75rem",
    width: "1.75rem",
  },
  addButton: {
    backgroundColor: {
      default: colors.card,
      ":hover": colors.background,
    },
    borderColor: colors.border,
    borderRadius: radii.full,
    boxShadow:
      "0 2px 6px rgb(87 83 78 / 0.08), 0 10px 18px -10px rgb(87 83 78 / 0.22)",
    color: colors.foreground,
    fontSize: "0.875rem",
    height: "auto",
    paddingBlock: "0.625rem",
    paddingInline: "1rem",
    width: "fit-content",
  },
  addIcon: {
    height: "1rem",
    marginRight: "0.5rem",
    width: "1rem",
  },
  deleteItem: {
    color: {
      default: "rgb(220 38 38)",
      ":focus": "rgb(220 38 38)",
    },
  },
  descriptionInput: {
    borderRadius: radii.xl,
    borderStyle: "solid",
    borderWidth: "1px",
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    minHeight: "100px",
    outline: {
      default: null,
      ":focus-visible": "none",
    },
    padding: "0.75rem",
    resize: "vertical",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color, border-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "100%",
  },
  disabledDescription: {
    backgroundColor: colors.muted,
  },
  dragHandle: {
    cursor: "move",
    left: "-1.25rem",
    opacity: {
      default: 0,
      ":is([data-section-item]:hover *)": 0.3,
      ":hover": 0.6,
    },
    position: "absolute",
    top: "0.625rem",
    transitionDuration: "150ms",
    transitionProperty: "opacity",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  dragIcon: {
    color: colors.mutedForeground,
    height: "1rem",
    width: "1rem",
  },
  fields: {
    display: "flex",
    flexDirection: "column",
    gap: "0.25rem",
    paddingRight: "2.25rem",
  },
  focusedDescription: {
    borderColor: "rgb(59 130 246)",
    boxShadow: `0 0 0 2px color-mix(in srgb, ${colors.primary} 20%, transparent)`,
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  idleDescription: {
    borderColor: colors.input,
  },
  item: {
    backgroundColor: colors.card,
    position: "relative",
  },
  itemActions: {
    opacity: {
      default: 0,
      ":is([data-section-item]:hover *)": 1,
    },
    position: "absolute",
    right: "0.5rem",
    top: "0.5rem",
    transitionDuration: "150ms",
    transitionProperty: "all",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  list: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
  },
  menuItem: {
    cursor: "pointer",
  },
  menuPanel: {
    overflow: "hidden",
    padding: "0.25rem",
  },
  root: {
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
  },
  titleInput: {
    "::placeholder": {
      color: `color-mix(in srgb, ${colors.mutedForeground} 60%, transparent)`,
    },
    backgroundColor: "transparent",
    borderWidth: 0,
    boxShadow: {
      default: "none",
      ":focus-visible": "none",
    },
    fontWeight: 500,
    padding: 0,
  },
});
