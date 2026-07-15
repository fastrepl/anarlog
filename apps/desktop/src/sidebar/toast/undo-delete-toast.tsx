import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useMemo } from "react";

import { sonnerToast } from "@hypr/ui/components/ui/toast";

import { restoreDeletedSession } from "~/session/queries";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { useTabs } from "~/store/zustand/tabs";
import { useUndoDelete } from "~/store/zustand/undo-delete";

type ToastGroup = {
  key: string;
  sessionIds: string[];
  isBatch: boolean;
  addedAt: number;
};

function useToastGroups(): ToastGroup[] {
  const pendingDeletions = useUndoDelete((state) => state.pendingDeletions);

  return useMemo(() => {
    const batchMap = new Map<string, string[]>();
    const singles: { sessionId: string; addedAt: number }[] = [];

    for (const [sessionId, pending] of Object.entries(pendingDeletions)) {
      if (pending.batchId) {
        const existing = batchMap.get(pending.batchId) ?? [];
        existing.push(sessionId);
        batchMap.set(pending.batchId, existing);
      } else {
        singles.push({ sessionId, addedAt: pending.addedAt });
      }
    }

    const groups: ToastGroup[] = singles.map(({ sessionId, addedAt }) => ({
      key: sessionId,
      sessionIds: [sessionId],
      isBatch: false,
      addedAt,
    }));

    for (const [batchId, sessionIds] of batchMap) {
      groups.push({
        key: batchId,
        sessionIds,
        isBatch: true,
        addedAt: Math.min(
          ...sessionIds.map((id) => pendingDeletions[id].addedAt),
        ),
      });
    }

    return groups.sort((a, b) => a.addedAt - b.addedAt);
  }, [pendingDeletions]);
}

function useRestoreGroup() {
  const queryClient = useQueryClient();
  const pendingDeletions = useUndoDelete((state) => state.pendingDeletions);
  const clearDeletion = useUndoDelete((state) => state.clearDeletion);
  const clearBatch = useUndoDelete((state) => state.clearBatch);
  const openCurrent = useTabs((state) => state.openCurrent);

  return useCallback(
    (group: ToastGroup) => {
      void (async () => {
        for (const sessionId of group.sessionIds) {
          const pending = pendingDeletions[sessionId];
          if (!pending) continue;
          await restoreDeletedSession(pending.data);
          void queryClient.invalidateQueries({
            predicate: (query) =>
              query.queryKey.length >= 2 &&
              query.queryKey[0] === "audio" &&
              query.queryKey[1] === sessionId,
          });
        }

        if (group.sessionIds.length > 0) {
          openCurrent({ type: "sessions", id: group.sessionIds[0] });
        }

        if (group.isBatch) {
          clearBatch(group.key);
        } else {
          clearDeletion(group.sessionIds[0]);
        }
      })().catch((error) => {
        console.error("[undo-delete] failed to restore session", error);
        sonnerToast.error("Could not restore deleted note");
      });
    },
    [pendingDeletions, openCurrent, clearDeletion, clearBatch, queryClient],
  );
}

function useConfirmGroup() {
  const confirmDeletion = useUndoDelete((state) => state.confirmDeletion);
  const confirmBatch = useUndoDelete((state) => state.confirmBatch);

  return useCallback(
    (group: ToastGroup) => {
      if (group.isBatch) {
        confirmBatch(group.key);
      } else {
        confirmDeletion(group.sessionIds[0]);
      }
    },
    [confirmDeletion, confirmBatch],
  );
}

export function UndoDeleteToast() {
  const groups = useToastGroups();

  return groups.map((group) => (
    <UndoDeleteSonnerToast
      key={`${group.key}:${group.sessionIds.join(":")}`}
      group={group}
    />
  ));
}

function UndoDeleteSonnerToast({ group }: { group: ToastGroup }) {
  const restoreGroup = useRestoreGroup();
  const confirmGroup = useConfirmGroup();
  const pendingDeletions = useUndoDelete((state) => state.pendingDeletions);
  const title = group.isBatch
    ? null
    : pendingDeletions[group.sessionIds[0]]?.data.session.title || "Untitled";
  const label = group.isBatch
    ? `Deleting ${group.sessionIds.length} notes`
    : `Deleting ${title}`;

  useMountEffect(() => {
    const toastId = `undo-delete:${group.key}`;

    sonnerToast.message(label, {
      id: toastId,
      duration: Infinity,
      action: {
        label: "Undo",
        onClick: () => restoreGroup(group),
      },
      cancel: {
        label: "Delete",
        onClick: () => confirmGroup(group),
      },
    });

    return () => sonnerToast.dismiss(toastId);
  });

  return null;
}
