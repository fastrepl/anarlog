import { useCallback } from "react";

import type { ContextEntity, ContextRef } from "~/chat/context/entities";
import { hydrateSessionContextFromFs } from "~/chat/context/session-context-hydrator";
import type { ResolvedChatContext } from "~/chat/transport";
import { type MainStore, useMainStore } from "~/session/hooks/storage";

function renderHumanContext(store: MainStore, humanId: string): string | null {
  const human = store.getRow("humans", humanId);
  const orgId = typeof human.org_id === "string" ? human.org_id : "";
  const organization =
    orgId && store.hasRow("organizations", orgId)
      ? store.getRow("organizations", orgId)
      : {};

  const name =
    typeof human.name === "string" && human.name.trim() ? human.name : null;
  const email =
    typeof human.email === "string" && human.email.trim() ? human.email : null;
  const jobTitle =
    typeof human.job_title === "string" && human.job_title.trim()
      ? human.job_title
      : null;
  const organizationName =
    typeof organization.name === "string" && organization.name.trim()
      ? organization.name
      : null;
  const memo =
    typeof human.memo === "string" && human.memo.trim() ? human.memo : null;

  if (!name && !email) return null;

  const details = [
    jobTitle,
    organizationName ? `Organization: ${organizationName}` : null,
    email ? `Email: ${email}` : null,
    memo ? `Notes: ${memo}` : null,
  ].filter(Boolean);

  return [`Referenced contact: ${name ?? email}`, ...details].join("\n");
}

function renderOrganizationContext(
  store: MainStore,
  organizationId: string,
): string | null {
  const organization = store.getRow("organizations", organizationId);
  const name =
    typeof organization.name === "string" && organization.name.trim()
      ? organization.name
      : null;

  return name ? `Referenced organization: ${name}` : null;
}

function getSessionDisplayData(
  store: MainStore | undefined,
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
  store: MainStore | undefined,
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
  store: MainStore | undefined,
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
  store: MainStore | undefined,
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

export function useDisplayEntityRenderer() {
  const store = useMainStore();
  return useCallback(
    (ref: ContextRef, removable: boolean): ContextEntity =>
      toDisplayEntity(ref, store, removable),
    [store],
  );
}

export function useResolveContextRef() {
  const store = useMainStore();
  return useCallback(
    async (ref: ContextRef): Promise<ResolvedChatContext | null> => {
      if (!store) return null;

      if (ref.kind === "session") {
        const context = await hydrateSessionContextFromFs(store, ref.sessionId);
        return context
          ? ({ kind: "session", context } satisfies ResolvedChatContext)
          : null;
      }

      if (ref.kind === "human") {
        const text = renderHumanContext(store, ref.humanId);
        return text
          ? ({ kind: "text", text } satisfies ResolvedChatContext)
          : null;
      }

      const text = renderOrganizationContext(store, ref.organizationId);
      return text
        ? ({ kind: "text", text } satisfies ResolvedChatContext)
        : null;
    },
    [store],
  );
}
