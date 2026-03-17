import { useCallback, useMemo, useRef } from "react";

import * as main from "~/store/tinybase/store/main";

export function useTranscriptEditing({
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
