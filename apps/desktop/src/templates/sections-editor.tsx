import {
  GripVertical as HandleIcon,
  MoreHorizontalIcon,
  Plus,
} from "lucide-react";
import { Reorder, useDragControls } from "motion/react";
import { useCallback, useEffect, useRef, useState } from "react";

import type { TemplateSection } from "@hypr/store";
import { Button } from "@hypr/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@hypr/ui/components/ui/dropdown-menu";
import { Input } from "@hypr/ui/components/ui/input";
import { cn } from "@hypr/utils";

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
    <div className="flex flex-col gap-3">
      <Reorder.Group values={drafts} onReorder={reorderSections}>
        <div className="flex flex-col gap-2">
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
          className="border-border bg-card text-foreground hover:bg-background h-auto w-fit rounded-full px-4 py-2.5 text-sm shadow-[0_2px_6px_rgba(87,83,78,0.08),0_10px_18px_-10px_rgba(87,83,78,0.22)]"
          onClick={addSection}
          disabled={disabled}
        >
          <Plus className="mr-2 h-4 w-4" />
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
  const [isFocused, setIsFocused] = useState(false);

  return (
    <div className="group bg-card relative">
      {!disabled && (
        <button
          type="button"
          className="absolute top-2.5 -left-5 cursor-move opacity-0 transition-opacity group-hover:opacity-30 hover:opacity-60"
          onPointerDown={(event) => dragControls.start(event)}
          disabled={disabled}
        >
          <HandleIcon className="text-muted-foreground h-4 w-4" />
        </button>
      )}

      {!disabled && (
        <div className="absolute top-2 right-2 opacity-0 transition-all group-hover:opacity-100">
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                type="button"
                size="icon"
                variant="ghost"
                className="text-muted-foreground hover:text-muted-foreground h-7 w-7"
                aria-label="Section actions"
              >
                <MoreHorizontalIcon className="size-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent variant="app" align="end">
              <AppFloatingPanel className="overflow-hidden p-1">
                <DropdownMenuItem
                  onClick={() => onInsertAbove(index)}
                  className="cursor-pointer"
                >
                  Insert above
                </DropdownMenuItem>
                <DropdownMenuItem
                  onClick={() => onInsertBelow(index + 1)}
                  className="cursor-pointer"
                >
                  Insert below
                </DropdownMenuItem>
                <DropdownMenuItem
                  onClick={() => onMove(item.key, -1)}
                  disabled={index === 0}
                  className="cursor-pointer"
                >
                  Move up
                </DropdownMenuItem>
                <DropdownMenuItem
                  onClick={() => onMove(item.key, 1)}
                  disabled={index === total - 1}
                  className="cursor-pointer"
                >
                  Move down
                </DropdownMenuItem>
                <DropdownMenuItem
                  onClick={() => onDelete(item.key)}
                  className="cursor-pointer text-red-600 focus:text-red-600"
                >
                  Delete
                </DropdownMenuItem>
              </AppFloatingPanel>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      )}

      <div className="flex flex-col gap-1 pr-9">
        <Input
          disabled={disabled}
          value={item.title}
          onChange={(e) => onChange({ ...item, title: e.target.value })}
          placeholder="Untitled"
          className="placeholder:text-muted-foreground/60 border-0 bg-transparent p-0 font-medium shadow-none focus-visible:ring-0 focus-visible:ring-offset-0"
        />

        <textarea
          disabled={disabled}
          value={item.description}
          onChange={(e) => onChange({ ...item, description: e.target.value })}
          placeholder="Template content with Jinja2: {{ variable }}, {% if condition %}"
          onFocus={() => setIsFocused(true)}
          onBlur={() => setIsFocused(false)}
          className={cn([
            "min-h-[100px] w-full resize-y rounded-xl border p-3 text-sm transition-colors",
            "focus-visible:outline-hidden",
            disabled
              ? "bg-muted"
              : isFocused
                ? "ring-primary/20 border-blue-500 ring-2"
                : "border-input",
          ])}
        />
      </div>
    </div>
  );
}
