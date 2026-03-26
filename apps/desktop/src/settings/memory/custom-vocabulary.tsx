import { useForm } from "@tanstack/react-form";
import {
  Check,
  CornerDownLeft,
  MinusCircle,
  Pencil,
  Plus,
  Search,
  X,
} from "lucide-react";
import { useMemo, useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

import * as main from "~/store/tinybase/store/main";

interface VocabItem {
  text: string;
  rowId: string;
}

export function CustomVocabularyView() {
  const vocabItems = useVocabs();
  const mutations = useVocabMutations();
  const [editingId, setEditingId] = useState<string | null>(null);
  const [searchValue, setSearchValue] = useState("");

  const form = useForm({
    defaultValues: {
      search: "",
    },
    onSubmit: ({ value }) => {
      const text = value.search.trim();
      if (text) {
        const allTexts = vocabItems.map((item) => item.text.toLowerCase());
        if (allTexts.includes(text.toLowerCase())) {
          return;
        }
        mutations.create(text);
        form.reset();
        setSearchValue("");
      }
    },
  });

  const filteredItems = useMemo(() => {
    if (!searchValue.trim()) {
      return vocabItems;
    }
    const query = searchValue.toLowerCase();
    return vocabItems.filter((item) => item.text.toLowerCase().includes(query));
  }, [vocabItems, searchValue]);

  const itemIndexMap = useMemo(() => {
    return new Map(vocabItems.map((item, index) => [item.rowId, index + 1]));
  }, [vocabItems]);

  const allTexts = vocabItems.map((item) => item.text.toLowerCase());
  const exactMatch = allTexts.includes(searchValue.trim().toLowerCase());
  const showAddEntry = searchValue.trim() && !exactMatch;

  return (
    <div className="flex flex-col gap-3">
      <h3 className="text-md font-serif font-semibold">Custom vocabulary</h3>

      <div className="overflow-hidden rounded-xl border border-neutral-200 bg-white">
        <form
          onSubmit={(e) => {
            e.preventDefault();
            e.stopPropagation();
            form.handleSubmit();
          }}
          className="flex h-9 items-center gap-2 border-b border-neutral-200 bg-stone-50 px-3"
        >
          <Search className="size-4 text-neutral-400" />
          <form.Field name="search">
            {(field) => (
              <input
                type="text"
                value={field.state.value}
                onChange={(e) => {
                  field.handleChange(e.target.value);
                  setSearchValue(e.target.value);
                }}
                placeholder="Search or add custom vocabulary"
                className="flex-1 bg-transparent text-sm text-neutral-900 placeholder:text-neutral-500 focus:outline-none"
              />
            )}
          </form.Field>
        </form>

        <div className="max-h-[300px] overflow-y-auto">
          {showAddEntry && (
            <button
              type="button"
              onClick={() => form.handleSubmit()}
              className={cn([
                "flex w-full items-center justify-between px-4 py-2.5",
                "border-b border-neutral-100",
                "group transition-colors hover:bg-neutral-50",
              ])}
            >
              <div className="flex items-center gap-2">
                <Plus className="size-3.5 text-neutral-400" />
                <span className="text-sm text-neutral-700">
                  Add "<span className="font-medium">{searchValue.trim()}</span>
                  "
                </span>
              </div>
              <span className="opacity-0 transition-opacity group-hover:opacity-100">
                <CornerDownLeft className="size-3.5 text-neutral-400" />
              </span>
            </button>
          )}
          {filteredItems.length === 0 && !showAddEntry ? (
            <div className="px-4 py-8 text-center text-sm text-neutral-400">
              No custom vocabulary added
            </div>
          ) : (
            filteredItems.map((item: VocabItem) => (
              <VocabularyItem
                key={item.rowId}
                item={item}
                itemNumber={itemIndexMap.get(item.rowId)!}
                vocabItems={vocabItems}
                isEditing={editingId === item.rowId}
                isSearching={searchValue.trim().length > 0}
                onStartEdit={() => setEditingId(item.rowId)}
                onCancelEdit={() => setEditingId(null)}
                onUpdate={mutations.update}
                onRemove={() => mutations.delete(item.rowId)}
              />
            ))
          )}
        </div>
      </div>
    </div>
  );
}

interface VocabularyItemProps {
  item: VocabItem;
  itemNumber: number;
  vocabItems: VocabItem[];
  isEditing: boolean;
  isSearching: boolean;
  onStartEdit: () => void;
  onCancelEdit: () => void;
  onUpdate: (rowId: string, text: string) => void;
  onRemove: () => void;
}

function VocabularyItem({
  item,
  itemNumber,
  vocabItems,
  isEditing,
  isSearching,
  onStartEdit,
  onCancelEdit,
  onUpdate,
  onRemove,
}: VocabularyItemProps) {
  const [hoveredItem, setHoveredItem] = useState(false);

  const form = useForm({
    defaultValues: {
      text: item.text,
    },
    onSubmit: ({ value }) => {
      const text = value.text.trim();
      if (text && text !== item.text) {
        onUpdate(item.rowId, text);
      }
      onCancelEdit();
    },
    validators: {
      onChange: ({ value }) => {
        const text = value.text.trim();
        if (!text) {
          return {
            fields: {
              text: "Vocabulary term cannot be empty",
            },
          };
        }
        const isDuplicate = vocabItems.some(
          (v) =>
            v.rowId !== item.rowId &&
            v.text.toLowerCase() === text.toLowerCase(),
        );
        if (isDuplicate) {
          return {
            fields: {
              text: "This term already exists",
            },
          };
        }
        return undefined;
      },
    },
  });

  return (
    <div
      className={cn([
        "flex items-center justify-between border-b border-neutral-100 px-4 py-3 last:border-b-0",
        !isEditing && "transition-colors hover:bg-neutral-50",
      ])}
      onMouseEnter={() => setHoveredItem(true)}
      onMouseLeave={() => setHoveredItem(false)}
    >
      <div className="flex min-w-0 flex-1 items-center gap-2">
        <span
          className={cn([
            "w-4 flex-shrink-0 text-center text-sm text-neutral-400",
            isSearching && "invisible",
          ])}
        >
          {itemNumber}
        </span>
        {isEditing ? (
          <form.Field name="text">
            {(field) => (
              <input
                type="text"
                value={field.state.value}
                onChange={(e) => field.handleChange(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    form.handleSubmit();
                  } else if (e.key === "Escape") {
                    e.preventDefault();
                    onCancelEdit();
                  }
                }}
                className="flex-1 bg-transparent text-sm text-neutral-900 focus:outline-none"
                autoFocus
              />
            )}
          </form.Field>
        ) : (
          <span className="text-sm text-neutral-700">{item.text}</span>
        )}
      </div>
      <div className="flex items-center gap-1">
        {isEditing ? (
          <form.Subscribe selector={(state) => [state.canSubmit]}>
            {([canSubmit]) => (
              <>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  onClick={() => form.handleSubmit()}
                  disabled={!canSubmit}
                  className="h-auto p-0 hover:bg-transparent disabled:opacity-50"
                >
                  <Check className="h-5 w-5 text-green-600" />
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  onClick={onCancelEdit}
                  className="h-auto p-0 hover:bg-transparent"
                >
                  <X className="h-5 w-5 text-neutral-500" />
                </Button>
              </>
            )}
          </form.Subscribe>
        ) : (
          hoveredItem && (
            <>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={onStartEdit}
                className="h-auto p-0 hover:bg-transparent"
              >
                <Pencil className="h-4 w-4 text-neutral-500" />
              </Button>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={onRemove}
                className="h-auto p-0 hover:bg-transparent"
              >
                <MinusCircle className="h-5 w-5 text-red-500" />
              </Button>
            </>
          )
        )}
      </div>
    </div>
  );
}

function useVocabs() {
  const table = main.UI.useResultTable(
    main.QUERIES.visibleVocabs,
    main.STORE_ID,
  );

  return Object.entries(table ?? {}).map(
    ([rowId, { text }]) =>
      ({
        rowId,
        text,
      }) as VocabItem,
  );
}

function useVocabMutations() {
  const { user_id } = main.UI.useValues(main.STORE_ID);

  const createRow = main.UI.useSetRowCallback(
    "memories",
    () => crypto.randomUUID(),
    (text: string) => ({
      user_id: user_id!,
      type: "vocab",
      text,
      created_at: new Date().toISOString(),
    }),
    [user_id],
    main.STORE_ID,
  );

  const updateRow = main.UI.useSetPartialRowCallback(
    "memories",
    ({ rowId }: { rowId: string; text: string }) => rowId,
    ({ text }: { rowId: string; text: string }) => ({ text }),
    [],
    main.STORE_ID,
  ) as (args: { rowId: string; text: string }) => void;

  const deleteRow = main.UI.useDelRowCallback(
    "memories",
    (rowId: string) => rowId,
    main.STORE_ID,
  );

  return {
    create: (text: string) => {
      if (!user_id) return;
      createRow(text);
    },
    update: (rowId: string, text: string) => {
      updateRow({ rowId, text });
    },
    delete: (rowId: string) => {
      deleteRow(rowId);
    },
  };
}
