import { useQueryClient } from "@tanstack/react-query";
import { type CSSProperties, useCallback, useMemo } from "react";

import { sonnerToast } from "@hypr/ui/components/ui/toast";

import { restoreDeletedSession } from "~/session/queries";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { useTabs } from "~/store/zustand/tabs";
import { UNDO_TIMEOUT_MS, useUndoDelete } from "~/store/zustand/undo-delete";

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
  const addDeletion = useUndoDelete((state) => state.addDeletion);
  const clearDeletion = useUndoDelete((state) => state.clearDeletion);
  const clearBatch = useUndoDelete((state) => state.clearBatch);
  const openCurrent = useTabs((state) => state.openCurrent);
  const invalidateResource = useTabs((state) => state.invalidateResource);

  return useCallback(
    (group: ToastGroup) => {
      const entries = group.sessionIds
        .map((sessionId) => pendingDeletions[sessionId])
        .filter((pending) => pending !== undefined);

      // Optimistic: dismiss the toast (also cancelling the finalize timers)
      // and reopen the tab before the restore writes commit.
      if (group.isBatch) {
        clearBatch(group.key);
      } else {
        clearDeletion(group.sessionIds[0]);
      }
      if (group.sessionIds.length > 0) {
        openCurrent({ type: "sessions", id: group.sessionIds[0] });
      }

      void (async () => {
        const remaining = [...entries];
        try {
          while (remaining.length > 0) {
            const pending = remaining[0];
            await restoreDeletedSession(pending.data);
            remaining.shift();
            const sessionId = pending.data.session.id;
            void queryClient.invalidateQueries({
              predicate: (query) =>
                query.queryKey.length >= 2 &&
                query.queryKey[0] === "audio" &&
                query.queryKey[1] === sessionId,
            });
          }
        } catch (error) {
          console.error("[undo-delete] failed to restore session", error);
          sonnerToast.error("Could not restore deleted note");
          // Re-add the unrestored deletions so their undo toast (and the
          // finalize path) comes back instead of leaving them tombstoned,
          // and close the optimistically reopened tab — it still points at
          // a tombstoned note.
          for (const pending of remaining) {
            addDeletion(
              pending.data,
              pending.onDeleteConfirm ?? undefined,
              pending.batchId ?? undefined,
            );
            invalidateResource("sessions", pending.data.session.id);
          }
        }
      })();
    },
    [
      pendingDeletions,
      openCurrent,
      invalidateResource,
      addDeletion,
      clearDeletion,
      clearBatch,
      queryClient,
    ],
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
  const pendingDeletions = useUndoDelete((state) => state.pendingDeletions);
  const title = group.isBatch
    ? null
    : pendingDeletions[group.sessionIds[0]]?.data.session.title || "Untitled";
  const noteLabel = group.sessionIds.length === 1 ? "note" : "notes";
  const label = group.isBatch
    ? `${group.sessionIds.length} ${noteLabel} deleted`
    : `${title} deleted`;
  const remainingDuration = Math.max(
    0,
    group.addedAt + UNDO_TIMEOUT_MS - Date.now(),
  );
  const progress = remainingDuration / UNDO_TIMEOUT_MS;

  useMountEffect(() => {
    const toastId = `undo-delete:${group.key}`;

    sonnerToast.message(label, {
      id: toastId,
      duration: Infinity,
      description: (
        <span
          aria-hidden="true"
          className="undo-delete-toast-gauge bg-primary block h-full w-full"
          style={
            {
              "--undo-delete-duration": `${remainingDuration}ms`,
              "--undo-delete-progress": progress,
            } as CSSProperties
          }
        />
      ),
      descriptionClassName:
        "bg-muted absolute inset-x-0 bottom-0 h-1 overflow-hidden",
      action: {
        label: "Undo",
        onClick: () => restoreGroup(group),
      },
    });

    return () => sonnerToast.dismiss(toastId);
  });

  return null;
}
