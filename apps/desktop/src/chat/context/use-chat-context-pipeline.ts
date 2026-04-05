import { useMemo } from "react";

import {
  type ContextEntity,
  type ContextRef,
  extractToolContextEntities,
  dedupeByKey,
} from "./entities";
import { extractContextRefsFromMessages } from "./refs";

import type { HyprUIMessage } from "~/chat/types";
import type * as main from "~/store/tinybase/store/main";

function getSessionDisplayData(
  store: ReturnType<typeof main.UI.useStore>,
  sessionId: string,
): { title: string | null; date: string | null } {
  if (!store) {
    return { title: null, date: null };
  }
  const row = store.getRow("sessions", sessionId);
  return {
    title: typeof row.title === "string" && row.title.trim() ? row.title : null,
    date:
      typeof row.created_at === "string" && row.created_at.trim()
        ? row.created_at
        : null,
  };
}

function getHumanDisplayData(
  store: ReturnType<typeof main.UI.useStore>,
  humanId: string,
): {
  name: string | null;
  email: string | null;
  organizationName: string | null;
} {
  if (!store) {
    return { name: null, email: null, organizationName: null };
  }

  const row = store.getRow("humans", humanId);
  const orgId = typeof row.org_id === "string" ? row.org_id : null;
  const organization =
    orgId && store.hasRow("organizations", orgId)
      ? store.getRow("organizations", orgId)
      : {};

  return {
    name: typeof row.name === "string" && row.name.trim() ? row.name : null,
    email: typeof row.email === "string" && row.email.trim() ? row.email : null,
    organizationName:
      typeof organization.name === "string" && organization.name.trim()
        ? organization.name
        : null,
  };
}

function getOrganizationDisplayData(
  store: ReturnType<typeof main.UI.useStore>,
  organizationId: string,
): { name: string | null } {
  if (!store) {
    return { name: null };
  }

  const row = store.getRow("organizations", organizationId);
  return {
    name: typeof row.name === "string" && row.name.trim() ? row.name : null,
  };
}

function toDisplayEntity(
  ref: ContextRef,
  store: ReturnType<typeof main.UI.useStore>,
  removable: boolean,
): ContextEntity {
  if (ref.kind === "session") {
    return {
      ...ref,
      ...getSessionDisplayData(store, ref.sessionId),
      removable,
    };
  }

  if (ref.kind === "human") {
    return {
      ...ref,
      ...getHumanDisplayData(store, ref.humanId),
      removable,
    };
  }

  return {
    ...ref,
    ...getOrganizationDisplayData(store, ref.organizationId),
    removable,
  };
}

type UseChatContextPipelineParams = {
  messages: HyprUIMessage[];
  currentSessionId?: string;
  pendingManualRefs: ContextRef[];
  store: ReturnType<typeof main.UI.useStore>;
};

export type DisplayEntity = ContextEntity & { pending: boolean };

export function useChatContextPipeline({
  messages,
  currentSessionId,
  pendingManualRefs,
  store,
}: UseChatContextPipelineParams): {
  contextEntities: DisplayEntity[];
  pendingRefs: ContextRef[];
} {
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
    () => committedRefs.map((ref) => toDisplayEntity(ref, store, false)),
    [committedRefs, store],
  );

  // Pending manual refs are removable; pending auto-current is not.
  const pendingEntities = useMemo(
    () =>
      pendingRefs.map((ref) =>
        toDisplayEntity(ref, store, ref.source === "manual"),
      ),
    [pendingRefs, store],
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
