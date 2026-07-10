import { useMemo } from "react";

import {
  type ContextEntity,
  type ContextRef,
  extractToolContextEntities,
  dedupeByKey,
} from "./entities";
import { extractContextRefsFromMessages } from "./refs";

import type { HyprUIMessage } from "~/chat/types";
import { useHumans, useOrganizations } from "~/contacts/queries";
import { useSessionSummaries } from "~/session/queries";

function getSessionDisplayData(
  sessions: ReturnType<typeof useSessionSummaries>,
  sessionId: string,
): { title: string | null; date: string | null } {
  const row = sessions.find((session) => session.id === sessionId);
  return {
    title: row?.title.trim() || null,
    date: row?.created_at.trim() || null,
  };
}

function getHumanDisplayData(
  humans: ReturnType<typeof useHumans>,
  organizations: ReturnType<typeof useOrganizations>,
  humanId: string,
): {
  name: string | null;
  email: string | null;
  organizationName: string | null;
} {
  const row = humans.find((human) => human.id === humanId);
  const organization = organizations.find(
    (candidate) => candidate.id === row?.organizationId,
  );

  return {
    name: row?.name.trim() || null,
    email: row?.email.trim() || null,
    organizationName: organization?.name.trim() || null,
  };
}

function getOrganizationDisplayData(
  organizations: ReturnType<typeof useOrganizations>,
  organizationId: string,
): { name: string | null } {
  const row = organizations.find(
    (organization) => organization.id === organizationId,
  );
  return {
    name: row?.name.trim() || null,
  };
}

function toDisplayEntity(
  ref: ContextRef,
  sessions: ReturnType<typeof useSessionSummaries>,
  humans: ReturnType<typeof useHumans>,
  organizations: ReturnType<typeof useOrganizations>,
  removable: boolean,
): ContextEntity {
  if (ref.kind === "session") {
    return {
      ...ref,
      ...getSessionDisplayData(sessions, ref.sessionId),
      removable,
    };
  }

  if (ref.kind === "human") {
    return {
      ...ref,
      ...getHumanDisplayData(humans, organizations, ref.humanId),
      removable,
    };
  }

  return {
    ...ref,
    ...getOrganizationDisplayData(organizations, ref.organizationId),
    removable,
  };
}

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
  const sessions = useSessionSummaries();
  const humans = useHumans();
  const organizations = useOrganizations();
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
    () =>
      committedRefs.map((ref) =>
        toDisplayEntity(ref, sessions, humans, organizations, false),
      ),
    [committedRefs, humans, organizations, sessions],
  );

  // Pending manual refs are removable; pending auto-current is not.
  const pendingEntities = useMemo(
    () =>
      pendingRefs.map((ref) =>
        toDisplayEntity(
          ref,
          sessions,
          humans,
          organizations,
          ref.source === "manual",
        ),
      ),
    [humans, organizations, pendingRefs, sessions],
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
