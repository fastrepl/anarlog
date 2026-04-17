import { useCallback } from "react";

import type { ContextRef } from "~/chat/context/entities";
import { hydrateSessionContextFromFs } from "~/chat/context/session-context-hydrator";
import type { ResolvedChatContext } from "~/chat/transport";
import * as main from "~/store/tinybase/store/main";

type Store = NonNullable<ReturnType<typeof main.UI.useStore>>;

function renderHumanContext(store: Store, humanId: string): string | null {
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
  store: Store,
  organizationId: string,
): string | null {
  const organization = store.getRow("organizations", organizationId);
  const name =
    typeof organization.name === "string" && organization.name.trim()
      ? organization.name
      : null;

  return name ? `Referenced organization: ${name}` : null;
}

export function useResolveContextRef() {
  const store = main.UI.useStore(main.STORE_ID);
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
