import { GripVertical as HandleIcon, Plus, X } from "lucide-react";
import { Reorder, useDragControls } from "motion/react";
import { useCallback, useEffect, useState } from "react";

import type { TemplateSection } from "@hypr/store";
import { Button } from "@hypr/ui/components/ui/button";
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
    reorderSections,
  };
}

export function SectionsList({
  disabled,
  items: _items,
  onChange,
}: {
  disabled: boolean;
  items: TemplateSection[];
  onChange: (items: TemplateSection[]) => void;
}) {
  const controls = useDragControls();
  const { drafts, addSection, changeSection, deleteSection, reorderSections } =
    useEditableSections({
      disabled,
      initialItems: _items,
      onChange,
    });

  return (
    <div className="flex flex-col gap-3">
      <Reorder.Group values={drafts} onReorder={reorderSections}>
        <div className="flex flex-col gap-2">
          {drafts.map((draft) => (
            <Reorder.Item key={draft.key} value={draft}>
              <SectionItem
                disabled={disabled}
                item={draft}
                onChange={changeSection}
                onDelete={deleteSection}
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
          className="w-full text-sm"
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
  item,
  onChange,
  onDelete,
  dragControls,
}: {
  disabled: boolean;
  item: SectionDraft;
  onChange: (item: SectionDraft) => void;
  onDelete: (key: string) => void;
  dragControls: ReturnType<typeof useDragControls>;
}) {
  const [isFocused, setIsFocused] = useState(false);

  return (
    <div className="group relative bg-white">
      {!disabled && (
        <button
          className="absolute top-2.5 -left-5 cursor-move opacity-0 transition-opacity group-hover:opacity-30 hover:opacity-60"
          onPointerDown={(event) => dragControls.start(event)}
          disabled={disabled}
        >
          <HandleIcon className="text-muted-foreground h-4 w-4" />
        </button>
      )}

      {!disabled && (
        <button
          className="absolute top-2 right-2 opacity-0 transition-all group-hover:opacity-30 hover:opacity-100"
          onClick={() => onDelete(item.key)}
          disabled={disabled}
        >
          <X size={16} />
        </button>
      )}

      <div className="flex flex-col gap-1">
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
