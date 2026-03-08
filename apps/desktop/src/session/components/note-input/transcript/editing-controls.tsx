import { useCallback, useMemo, useRef } from "react";

import { cn } from "@hypr/utils";

import * as main from "~/store/tinybase/store/main";

export function EditingControls({
  sessionId: _sessionId,
  isEditing,
  setIsEditing,
}: {
  sessionId: string;
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

function useTranscriptEditing({
  isEditing,
  setIsEditing,
}: {
  isEditing: boolean;
  setIsEditing: (isEditing: boolean) => void;
}) {
  const checkpoints = main.UI.useCheckpoints(main.STORE_ID);
  const checkpointIds = main.UI.useCheckpointIds(main.STORE_ID) ?? [
    [],
    undefined,
    [],
  ];
  const [, currentId, forwardIds] = checkpointIds;

  const baselineIdRef = useRef<string | undefined>(undefined);

  const canUndo = useMemo(
    () =>
      isEditing &&
      !!baselineIdRef.current &&
      !!currentId &&
      currentId !== baselineIdRef.current,
    [isEditing, currentId],
  );

  const canRedo = useMemo(
    () => isEditing && forwardIds.length > 0,
    [isEditing, forwardIds.length],
  );

  const handleUndo = useCallback(() => {
    if (canUndo && checkpoints) {
      checkpoints.goBackward();
    }
  }, [canUndo, checkpoints]);

  const handleRedo = useCallback(() => {
    if (canRedo && checkpoints) {
      checkpoints.goForward();
    }
  }, [canRedo, checkpoints]);

  const handleEdit = useCallback(() => {
    if (!checkpoints) {
      return;
    }
    const [, id] = checkpoints.getCheckpointIds();
    baselineIdRef.current =
      id ?? checkpoints.addCheckpoint("transcript_edit:baseline");
    setIsEditing(true);
  }, [checkpoints, setIsEditing]);

  const handleSave = useCallback(() => {
    if (!checkpoints) {
      return;
    }
    checkpoints.addCheckpoint("transcript_edit:save");
    baselineIdRef.current = undefined;
    setIsEditing(false);
  }, [checkpoints, setIsEditing]);

  const handleCancel = useCallback(() => {
    if (!checkpoints || baselineIdRef.current === undefined) {
      return;
    }
    checkpoints.goTo(baselineIdRef.current);
    baselineIdRef.current = undefined;
    setIsEditing(false);
  }, [checkpoints, setIsEditing]);

  return {
    canUndo,
    canRedo,
    handleUndo,
    handleRedo,
    handleEdit,
    handleSave,
    handleCancel,
  };
}
