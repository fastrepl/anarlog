import { cn } from "@hypr/utils";

import { useTranscriptEditing } from "./use-transcript-editing";

export function EditingControls({
  isEditing,
  setIsEditing,
}: {
  isEditing: boolean;
  setIsEditing: (isEditing: boolean) => void;
}) {
  const {
    canUndo,
    canRedo,
    handleUndo,
    handleRedo,
    handleEdit,
    handleSave,
    handleCancel,
  } = useTranscriptEditing({
    isEditing,
    setIsEditing,
  });

  const viewModeControls = (
    <button
      onClick={handleEdit}
      className={cn([
        "rounded-xs px-3 py-0.5 text-xs",
        "bg-neutral-100 text-neutral-900 hover:bg-neutral-200",
        "transition-colors",
      ])}
    >
      Edit
    </button>
  );

  const editModeControls = (
    <>
      <button
        onClick={handleUndo}
        disabled={!canUndo}
        className={cn([
          "rounded-xs px-3 py-0.5 text-xs",
          "transition-colors",
          canUndo
            ? "bg-neutral-100 text-neutral-900 hover:bg-neutral-200"
            : "cursor-not-allowed bg-neutral-50 text-neutral-400",
        ])}
      >
        &lt;
      </button>
      <button
        onClick={handleRedo}
        disabled={!canRedo}
        className={cn([
          "rounded-xs px-3 py-0.5 text-xs",
          "transition-colors",
          canRedo
            ? "bg-neutral-100 text-neutral-900 hover:bg-neutral-200"
            : "cursor-not-allowed bg-neutral-50 text-neutral-400",
        ])}
      >
        &gt;
      </button>
      <button
        onClick={handleCancel}
        className={cn([
          "rounded-xs px-3 py-0.5 text-xs",
          "bg-neutral-100 text-neutral-900 hover:bg-neutral-200",
          "transition-colors",
        ])}
      >
        Cancel
      </button>
      <button
        onClick={handleSave}
        className={cn([
          "rounded-xs px-3 py-0.5 text-xs",
          "bg-neutral-900 text-white hover:bg-neutral-800",
          "transition-colors",
        ])}
      >
        Save
      </button>
    </>
  );

  return (
    <div className={cn(["my-2 flex items-center gap-2"])}>
      <div className="flex-1" />
      {isEditing ? editModeControls : viewModeControls}
    </div>
  );
}
