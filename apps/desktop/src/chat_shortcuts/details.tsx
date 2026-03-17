import { useCallback, useEffect, useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import { Input } from "@hypr/ui/components/ui/input";
import { Textarea } from "@hypr/ui/components/ui/textarea";

import {
  DangerZone,
  ResourceDetailEmpty,
  ResourcePreviewHeader,
} from "~/shared/ui/resource-list";
import * as main from "~/store/tinybase/store/main";

type WebShortcut = {
  slug: string;
  title: string;
  description: string;
  category: string;
  targets?: string[];
  prompt: string;
};

export function ChatShortcutDetailsColumn({
  isWebMode,
  selectedMineId,
  selectedWebShortcut,
  setSelectedMineId,
  handleCloneShortcut,
}: {
  isWebMode: boolean;
  selectedMineId: string | null;
  selectedWebShortcut: WebShortcut | null;
  setSelectedMineId: (id: string | null) => void;
  handleCloneShortcut: (shortcut: WebShortcut) => void;
}) {
  if (isWebMode) {
    if (!selectedWebShortcut) {
      return <ResourceDetailEmpty message="Select a shortcut to preview" />;
    }
    return (
      <WebShortcutPreview
        shortcut={selectedWebShortcut}
        onClone={handleCloneShortcut}
      />
    );
  }

  if (!selectedMineId) {
    return <ResourceDetailEmpty message="Select a shortcut to view or edit" />;
  }

  return (
    <ChatShortcutForm
      key={selectedMineId}
      id={selectedMineId}
      setSelectedMineId={setSelectedMineId}
    />
  );
}

function WebShortcutPreview({
  shortcut,
  onClone,
}: {
  shortcut: WebShortcut;
  onClone: (shortcut: WebShortcut) => void;
}) {
  return (
    <div className="flex h-full flex-col">
      <ResourcePreviewHeader
        title={shortcut.title}
        description={shortcut.description}
        category={shortcut.category}
        targets={shortcut.targets}
        onClone={() => onClone(shortcut)}
      />

      <div className="flex-1 p-6">
        <h3 className="mb-3 text-sm font-medium text-neutral-600">
          Prompt Content
        </h3>
        <div className="rounded-lg border border-neutral-200 bg-neutral-50 p-4">
          <p className="text-sm whitespace-pre-wrap text-neutral-700">
            {shortcut.prompt}
          </p>
        </div>
      </div>
    </div>
  );
}

function ChatShortcutForm({
  id,
  setSelectedMineId,
}: {
  id: string;
  setSelectedMineId: (id: string | null) => void;
}) {
  const title = main.UI.useCell("chat_shortcuts", id, "title", main.STORE_ID);
  const content = main.UI.useCell(
    "chat_shortcuts",
    id,
    "content",
    main.STORE_ID,
  );
  const [localTitle, setLocalTitle] = useState(title || "");
  const [localContent, setLocalContent] = useState(content || "");

  useEffect(() => {
    setLocalTitle(title || "");
    setLocalContent(content || "");
  }, [title, content, id]);

  const handleUpdate = main.UI.useSetPartialRowCallback(
    "chat_shortcuts",
    id,
    (row: { title?: string; content?: string }) => row,
    [id],
    main.STORE_ID,
  );

  const handleDelete = main.UI.useDelRowCallback(
    "chat_shortcuts",
    () => id,
    main.STORE_ID,
  );

  const handleSave = useCallback(() => {
    handleUpdate({ title: localTitle, content: localContent });
  }, [handleUpdate, localTitle, localContent]);

  const handleDeleteClick = useCallback(() => {
    handleDelete();
    setSelectedMineId(null);
  }, [handleDelete, setSelectedMineId]);

  const hasChanges =
    localTitle !== (title || "") || localContent !== (content || "");

  return (
    <div className="flex h-full flex-col">
      <div className="border-b border-neutral-200 px-6 py-4">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-lg font-semibold">Edit Shortcut</h2>
            <p className="mt-1 text-sm text-neutral-500">
              Create a quick shortcut for chat inputs
            </p>
          </div>
          <div className="flex gap-2">
            <Button size="sm" onClick={handleSave} disabled={!hasChanges}>
              Save
            </Button>
          </div>
        </div>
      </div>

      <div className="flex flex-1 flex-col gap-4 p-6">
        <div>
          <label className="mb-1.5 block text-sm font-medium text-neutral-700">
            Title
          </label>
          <Input
            value={localTitle}
            onChange={(e) => setLocalTitle(e.target.value)}
            placeholder="Enter a title for this shortcut..."
          />
        </div>
        <div>
          <label className="mb-1.5 block text-sm font-medium text-neutral-700">
            Content
          </label>
          <Textarea
            value={localContent}
            onChange={(e) => setLocalContent(e.target.value)}
            placeholder="Enter your chat shortcut content..."
            className="min-h-[200px] resize-none"
          />
        </div>
      </div>

      <div className="border-t border-neutral-200 p-6">
        <DangerZone
          title="Delete this shortcut"
          description="This action cannot be undone"
          buttonLabel="Delete Shortcut"
          onAction={handleDeleteClick}
        />
      </div>
    </div>
  );
}
