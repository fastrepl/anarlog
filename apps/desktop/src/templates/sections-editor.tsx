import {
  GripVertical as HandleIcon,
  MoreHorizontalIcon,
  Plus,
} from "lucide-react";
import { Reorder, useDragControls } from "motion/react";
import { useCallback, useEffect, useState } from "react";

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

function createDraft(section: TemplateSection, key?: string): SectionDraft {
  return {
    key: key ?? crypto.randomUUID(),
    title: section.title,
    description: section.description,
  };
}

function toSection(draft: SectionDraft): TemplateSection {
  return {
    title: draft.title,
    description: draft.description,
  };
}

function sameSection(draft: SectionDraft, section?: TemplateSection) {
  if (!section) {
    return false;
  }
  return (
    draft.title === section.title && draft.description === section.description
  );
}

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
    initialItems.map((section) => createDraft(section)),
  );

  useEffect(() => {
    setDrafts((prev) => {
      const shouldUpdate =
        prev.length !== initialItems.length ||
        prev.some((draft, index) => !sameSection(draft, initialItems[index]));

      if (!shouldUpdate) {
        return prev;
      }

      return initialItems.map((section, index) =>
        createDraft(section, prev[index]?.key),
      );
    });
  }, [initialItems]);

  const commitDrafts = useCallback(
    (next: SectionDraft[] | ((prev: SectionDraft[]) => SectionDraft[])) => {
      setDrafts((prev) => {
        const resolved = typeof next === "function" ? next(prev) : next;
        onChange(resolved.map((draft) => toSection(draft)));
        return resolved;
      });
    },
    [onChange],
  );

  const changeSection = useCallback(
    (draft: SectionDraft) => {
      commitDrafts((prev) =>
        prev.map((section) => (section.key === draft.key ? draft : section)),
      );
    },
    [commitDrafts],
  );

  const deleteSection = useCallback(
    (key: string) => {
      commitDrafts((prev) => prev.filter((section) => section.key !== key));
    },
    [commitDrafts],
  );

  const insertSectionAt = useCallback(
    (index: number) => {
      commitDrafts((prev) => {
        const next = [...prev];
        next.splice(index, 0, createDraft({ title: "", description: "" }));
        return next;
      });
    },
    [commitDrafts],
  );

  const moveSection = useCallback(
    (key: string, direction: -1 | 1) => {
      commitDrafts((prev) => {
        const currentIndex = prev.findIndex((section) => section.key === key);
        const targetIndex = currentIndex + direction;

        if (currentIndex < 0 || targetIndex < 0 || targetIndex >= prev.length) {
          return prev;
        }

        const next = [...prev];
        const [section] = next.splice(currentIndex, 1);
        next.splice(targetIndex, 0, section);
        return next;
      });
    },
    [commitDrafts],
  );

  const reorderSections = useCallback(
    (next: SectionDraft[]) => {
      if (disabled) {
        return;
      }
      commitDrafts(next);
    },
    [commitDrafts, disabled],
  );

  const addSection = useCallback(() => {
    commitDrafts((prev) => [
      ...prev,
      createDraft({ title: "", description: "" }),
    ]);
  }, [commitDrafts]);

  return {
    drafts,
    addSection,
    changeSection,
    deleteSection,
    insertSectionAt,
    moveSection,
    reorderSections,
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
          className="h-auto w-fit rounded-full border-neutral-200 bg-white px-4 py-2.5 text-sm text-stone-800 shadow-[0_2px_6px_rgba(87,83,78,0.08),0_10px_18px_-10px_rgba(87,83,78,0.22)] hover:bg-stone-50"
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
    <div className="group relative bg-white">
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
                className="h-7 w-7 text-neutral-400 hover:text-neutral-700"
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
            "min-h-[100px] w-full resize-y rounded-xl border p-3 font-mono text-sm transition-colors",
            "focus-visible:outline-hidden",
            disabled
              ? "bg-neutral-50"
              : isFocused
                ? "ring-primary/20 border-blue-500 ring-2"
                : "border-input",
          ])}
        />
      </div>
    </div>
  );
}
