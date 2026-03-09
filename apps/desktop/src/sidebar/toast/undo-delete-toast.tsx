import { useQueryClient } from "@tanstack/react-query";
import { AnimatePresence, motion } from "motion/react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { useHotkeys } from "react-hotkeys-hook";

import { cn } from "@hypr/utils";

import { restoreSessionData } from "~/store/tinybase/store/deleteSession";
import * as main from "~/store/tinybase/store/main";
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

    const groups: ToastGroup[] = [];

    for (const { sessionId, addedAt } of singles) {
      groups.push({
        key: sessionId,
        sessionIds: [sessionId],
        isBatch: false,
        addedAt,
      });
    }

    for (const [batchId, sessionIds] of batchMap) {
      const earliest = Math.min(
        ...sessionIds.map((id) => pendingDeletions[id].addedAt),
      );
      groups.push({
        key: batchId,
        sessionIds,
        isBatch: true,
        addedAt: earliest,
      });
    }

    groups.sort((a, b) => a.addedAt - b.addedAt);
    return groups;
  }, [pendingDeletions]);
}

function useRestoreGroup() {
  const store = main.UI.useStore(main.STORE_ID);
  const queryClient = useQueryClient();
  const pendingDeletions = useUndoDelete((state) => state.pendingDeletions);
  const clearDeletion = useUndoDelete((state) => state.clearDeletion);
  const clearBatch = useUndoDelete((state) => state.clearBatch);
  const openCurrent = useTabs((state) => state.openCurrent);

  return useCallback(
    (group: ToastGroup) => {
      if (!store) return;

      for (const sessionId of group.sessionIds) {
        const pending = pendingDeletions[sessionId];
        if (!pending) continue;
        restoreSessionData(store, pending.data);
        void queryClient.invalidateQueries({
          predicate: (query) =>
            query.queryKey.length >= 2 &&
            query.queryKey[0] === "audio" &&
            query.queryKey[1] === sessionId,
        });
      }

      if (group.sessionIds.length > 0) {
        openCurrent({
          type: "sessions",
          id: group.sessionIds[0],
        });
      }

      if (group.isBatch) {
        clearBatch(group.key);
      } else {
        clearDeletion(group.sessionIds[0]);
      }
    },
    [
      store,
      pendingDeletions,
      openCurrent,
      clearDeletion,
      clearBatch,
      queryClient,
    ],
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

function useGroupCountdown(group: ToastGroup) {
  const pendingDeletions = useUndoDelete((state) => state.pendingDeletions);
  const [remaining, setRemaining] = useState(UNDO_TIMEOUT_MS);

  const { earliest, isPaused, frozenRemaining } = useMemo(() => {
    let min = Infinity;
    let paused = false;
    let frozen = 0;

    for (const id of group.sessionIds) {
      const p = pendingDeletions[id];
      if (!p) continue;
      min = Math.min(min, p.data.deletedAt);
      if (p.paused && p.pausedAt) {
        paused = true;
        const elapsed = p.pausedAt - p.data.deletedAt;
        frozen = Math.max(0, UNDO_TIMEOUT_MS - elapsed);
      }
    }

    return {
      earliest: min === Infinity ? Date.now() : min,
      isPaused: paused,
      frozenRemaining: frozen,
    };
  }, [group.sessionIds, pendingDeletions]);

  useEffect(() => {
    if (isPaused) {
      setRemaining(frozenRemaining);
      return;
    }

    const update = () => {
      const elapsed = Date.now() - earliest;
      setRemaining(Math.max(0, UNDO_TIMEOUT_MS - elapsed));
    };
    update();
    const id = setInterval(update, 100);
    return () => clearInterval(id);
  }, [earliest, isPaused, frozenRemaining]);

  return Math.ceil(remaining / 1000);
}

export function UndoDeleteKeyboardHandler() {
  const groups = useToastGroups();
  const restoreGroup = useRestoreGroup();

  const latestGroup = useMemo(() => {
    if (groups.length === 0) return null;
    return groups[groups.length - 1];
  }, [groups]);

  useHotkeys(
    "mod+z",
    () => {
      if (latestGroup) {
        restoreGroup(latestGroup);
      }
    },
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [latestGroup, restoreGroup],
  );

  return null;
}

export function UndoDeleteToast() {
  const groups = useToastGroups();

  return createPortal(
    <AnimatePresence mode="popLayout">
      {groups.map((group, index) => (
        <ToastPill
          key={group.key}
          group={group}
          index={index}
          total={groups.length}
        />
      ))}
    </AnimatePresence>,
    document.body,
  );
}

const SIDEBAR_SELECTOR = "[data-testid='main-app-shell']";

function ToastPill({
  group,
  index,
  total,
}: {
  group: ToastGroup;
  index: number;
  total: number;
}) {
  const restoreGroup = useRestoreGroup();
  const confirmGroup = useConfirmGroup();
  const seconds = useGroupCountdown(group);
  const pendingDeletions = useUndoDelete((state) => state.pendingDeletions);
  const pauseGroup = useUndoDelete((state) => state.pauseGroup);
  const resumeGroup = useUndoDelete((state) => state.resumeGroup);

  const [contentOffset, setContentOffset] = useState(0);

  useEffect(() => {
    const computeOffset = () => {
      const shell = document.querySelector(SIDEBAR_SELECTOR);
      if (!shell) {
        setContentOffset(0);
        return;
      }

      const panels = document.querySelectorAll("[data-panel-id]");
      const bodyPanel = panels[0];
      if (!bodyPanel) {
        setContentOffset(0);
        return;
      }

      const bodyRect = bodyPanel.getBoundingClientRect();
      const bodyCenter = bodyRect.left + bodyRect.width / 2;
      const windowCenter = window.innerWidth / 2;
      setContentOffset(bodyCenter - windowCenter);
    };

    computeOffset();
    window.addEventListener("resize", computeOffset);

    const resizeObserver = new ResizeObserver(computeOffset);
    const panels = document.querySelectorAll("[data-panel-id]");
    for (const panel of panels) {
      resizeObserver.observe(panel);
    }

    return () => {
      window.removeEventListener("resize", computeOffset);
      resizeObserver.disconnect();
    };
  }, []);

  const handleMouseEnter = useCallback(() => {
    pauseGroup(group.sessionIds);
  }, [pauseGroup, group.sessionIds]);

  const handleMouseLeave = useCallback(() => {
    resumeGroup(group.sessionIds);
  }, [resumeGroup, group.sessionIds]);

  const title = useMemo(() => {
    if (group.isBatch) return null;
    const p = pendingDeletions[group.sessionIds[0]];
    return p?.data.session.title || "Untitled";
  }, [group, pendingDeletions]);

  const stackOffset = (total - 1 - index) * 44;
  const isTop = index === total - 1;

  const count = group.sessionIds.length;
  const label = group.isBatch ? `Deleting ${count} notes` : `Deleting ${title}`;

  return (
    <motion.div
      layout
      initial={{ opacity: 0, y: 20, scale: 0.95 }}
      animate={{
        opacity: isTop ? 1 : 0.7,
        y: -stackOffset,
        scale: 1,
      }}
      exit={{ opacity: 0, y: 20, scale: 0.95 }}
      transition={{ duration: 0.2, ease: "easeOut" }}
      style={{
        zIndex: 50 + index,
        pointerEvents: isTop ? "auto" : "none",
        left: `calc(50% + ${contentOffset}px)`,
      }}
      className="fixed bottom-6 -translate-x-1/2"
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
    >
      <div
        className={cn([
          "flex items-center gap-3 py-1.5 pr-1.5 pl-4",
          "rounded-full bg-white",
          "border border-neutral-200 shadow-lg",
        ])}
      >
        <span className="max-w-[200px] truncate text-sm text-neutral-600">
          {label}...
        </span>

        <button
          onClick={() => confirmGroup(group)}
          className={cn([
            "rounded-full px-3 py-1.5 text-xs font-medium",
            "whitespace-nowrap",
            "bg-neutral-100 text-neutral-500",
            "hover:bg-red-100 hover:text-red-600",
            "transition-colors",
          ])}
        >
          yes
        </button>

        <button
          onClick={() => restoreGroup(group)}
          className={cn([
            "rounded-full px-3 py-1.5 text-xs font-medium",
            "whitespace-nowrap",
            "bg-neutral-900 text-white",
            "hover:bg-neutral-800",
            "transition-colors",
          ])}
        >
          undo {seconds}s
        </button>
      </div>
    </motion.div>
  );
}
