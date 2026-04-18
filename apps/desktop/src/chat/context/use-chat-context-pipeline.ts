import { useMemo } from "react";

import {
  type ContextEntity,
  type ContextRef,
  dedupeByKey,
  extractToolContextEntities,
} from "./entities";
import { extractContextRefsFromMessages } from "./refs";

import { useDisplayEntityRenderer } from "~/chat/hooks/context-renderers";
import type { HyprUIMessage } from "~/chat/types";

type UseChatContextPipelineParams = {
  messages: HyprUIMessage[];
  currentSessionId?: string;
  pendingManualRefs: ContextRef[];
};

export type DisplayEntity = ContextEntity & { pending: boolean };

export function useChatContextPipeline({
  messages,
  currentSessionId,
  pendingManualRefs,
}: UseChatContextPipelineParams): {
  contextEntities: DisplayEntity[];
  pendingRefs: ContextRef[];
} {
  const toDisplayEntity = useDisplayEntityRenderer();

  const committedRefs = useMemo(
    () => extractContextRefsFromMessages(messages),
    [messages],
  );

  const toolEntities = useMemo(
    () => extractToolContextEntities(messages),
    [messages],
  );

  // Refs that will be attached to the next message send.
  const pendingRefs = useMemo((): ContextRef[] => {
    const refs: ContextRef[] = [];
    if (currentSessionId) {
      refs.push({
        kind: "session",
        key: `session:auto:${currentSessionId}`,
        source: "auto-current",
        sessionId: currentSessionId,
      });
    }
    refs.push(...pendingManualRefs);
    return refs;
  }, [currentSessionId, pendingManualRefs]);

  const committedEntities = useMemo(
    () => committedRefs.map((ref) => toDisplayEntity(ref, false)),
    [committedRefs, toDisplayEntity],
  );

  // Pending manual refs are removable; pending auto-current is not.
  const pendingEntities = useMemo(
    () =>
      pendingRefs.map((ref) => toDisplayEntity(ref, ref.source === "manual")),
    [pendingRefs, toDisplayEntity],
  );

  const rawEntities = useMemo(
    () => dedupeByKey([committedEntities, toolEntities, pendingEntities]),
    [committedEntities, toolEntities, pendingEntities],
  );

  const committedKeys = useMemo(
    () => new Set(committedRefs.map((ref) => ref.key)),
    [committedRefs],
  );

  const pendingKeys = useMemo(
    () => new Set(pendingRefs.map((ref) => ref.key)),
    [pendingRefs],
  );

  const contextEntities: DisplayEntity[] = useMemo(
    () =>
      rawEntities.map((entity) => ({
        ...entity,
        pending: pendingKeys.has(entity.key) && !committedKeys.has(entity.key),
      })),
    [rawEntities, pendingKeys, committedKeys],
  );

  return { contextEntities, pendingRefs };
}
