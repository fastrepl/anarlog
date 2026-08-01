import { type MutableRefObject, useCallback, useRef } from "react";

import {
  type PublishedSessionShareSnapshot,
  ShareManagementError,
  type ShareManagementContext,
} from "./client";
import {
  requireManagementContext,
  ShareOperationAbortedError,
  type SharePanelIdentity,
} from "./management";

import { useAuth } from "~/auth";
import type { SharedNoteAttachment } from "~/shared-notes/cache";

export type RunShareOperation = <T>(
  operation: (signal: AbortSignal) => Promise<T>,
) => Promise<T>;

export type RequireActiveShareContext = (
  signal?: AbortSignal,
) => ShareManagementContext;

export type PublishLatestSessionShare = (
  signal?: AbortSignal,
  requestedAttachments?: SharedNoteAttachment[],
  localOverrides?: Map<string, string>,
  resolveConflict?: boolean,
) => Promise<PublishedSessionShareSnapshot>;

export function useShareOperationLifecycle({
  auth,
  identity,
  pendingRef,
}: {
  auth: ReturnType<typeof useAuth>;
  identity: SharePanelIdentity;
  pendingRef: MutableRefObject<boolean>;
}) {
  const latestAuthRef = useRef(auth);
  latestAuthRef.current = auth;
  const operationControllersRef = useRef(new Set<AbortController>());
  const operationLifecycleRef = useCallback(
    (node: HTMLDivElement | null) => {
      if (node) return;
      pendingRef.current = false;
      for (const controller of operationControllersRef.current) {
        controller.abort();
      }
      operationControllersRef.current.clear();
    },
    [pendingRef],
  );
  const runOperation: RunShareOperation = async (operation) => {
    const controller = new AbortController();
    operationControllersRef.current.add(controller);
    try {
      const result = await operation(controller.signal);
      if (controller.signal.aborted) throw new ShareOperationAbortedError();
      return result;
    } catch (error) {
      if (controller.signal.aborted) throw new ShareOperationAbortedError();
      throw error;
    } finally {
      operationControllersRef.current.delete(controller);
    }
  };
  const requireActiveContext: RequireActiveShareContext = (signal) => {
    if (signal?.aborted) throw new ShareManagementError();
    const context = requireManagementContext(latestAuthRef.current);
    if (context.session.user.id !== identity.ownerUserId) {
      throw new ShareManagementError();
    }
    return { ...context, signal };
  };

  return { operationLifecycleRef, runOperation, requireActiveContext };
}
