import { useRouteContext } from "@tanstack/react-router";
import { useCallback, useEffect, useRef } from "react";

import { useLanguageModel, useLLMConnection } from "~/ai/hooks";
import { getAllEvents } from "~/calendar/queries";
import { useSessionTab } from "~/chat/components/use-session-tab";
import { buildChatTools } from "~/chat/tools";
import { useRegisterTools } from "~/contexts/tool";
import { useSearchEngine } from "~/search/contexts/engine";
import { initEnhancerService } from "~/services/enhancer";
import { getSessionEvent } from "~/session/utils";
import { useDesktopTabLifecycle } from "~/shared/desktop-tab-lifecycle";
import * as main from "~/store/tinybase/store/main";
import * as settings from "~/store/tinybase/store/settings";
import { useTabs } from "~/store/zustand/tabs";

export function useClassicMainLifecycle() {
  const openNew = useTabs((state) => state.openNew);
  const store = main.UI.useStore(main.STORE_ID);
  const indexes = main.UI.useIndexes(main.STORE_ID);

  const openDefaultEmptyTab = useCallback(() => {
    openNew({ type: "empty" });
  }, [openNew]);

  useDesktopTabLifecycle({
    store,
    indexes,
    onEmpty: openDefaultEmptyTab,
    onZeroTabs: openDefaultEmptyTab,
  });
}

export function ClassicMainServices() {
  return (
    <>
      <ToolRegistration />
      <EnhancerInit />
    </>
  );
}

function ToolRegistration() {
  const { search } = useSearchEngine();
  const store = main.UI.useStore(main.STORE_ID);
  const indexes = main.UI.useIndexes(main.STORE_ID);
  const storeRef = useRef(store);
  storeRef.current = store;
  const indexesRef = useRef(indexes);
  indexesRef.current = indexes;

  const getContactSearchResults = useCallback(
    async (query: string, limit: number) => {
      if (!store) {
        return [];
      }

      const q = query.trim().toLowerCase();
      const rows: Array<{
        id: string;
        name: string;
        email: string | null;
        jobTitle: string | null;
        organization: string | null;
        memo: string | null;
        createdAt: number;
      }> = [];

      store.forEachRow("humans", (rowId, _forEachCell) => {
        const row = store.getRow("humans", rowId);
        if (!row) {
          return;
        }

        const orgId =
          typeof row.org_id === "string" && row.org_id ? row.org_id : null;
        const orgName = orgId
          ? (store.getCell("organizations", orgId, "name") as string | null)
          : null;

        const name = typeof row.name === "string" ? row.name : "";
        const email =
          typeof row.email === "string" && row.email ? row.email : null;
        const jobTitle =
          typeof row.job_title === "string" && row.job_title
            ? row.job_title
            : null;
        const memo = typeof row.memo === "string" && row.memo ? row.memo : null;

        const searchable = [name, email, jobTitle, memo, orgName]
          .filter(Boolean)
          .join("\n")
          .toLowerCase();

        if (q && !searchable.includes(q)) {
          return;
        }

        const createdAt = Date.parse((row.created_at as string) || "") || 0;

        rows.push({
          id: rowId,
          name,
          email,
          jobTitle,
          organization: orgName,
          memo,
          createdAt,
        });
      });

      rows.sort((a, b) => b.createdAt - a.createdAt);

      return rows
        .slice(0, limit)
        .map(({ createdAt: _createdAt, ...row }) => row);
    },
    [store],
  );

  const getCalendarEventSearchResults = useCallback(
    async (query: string, limit: number) => {
      if (!store) {
        return [];
      }

      const q = query.trim().toLowerCase();
      const sessionByTrackingId = new Map<string, string>();

      store.forEachRow("sessions", (sessionId, _forEachCell) => {
        const row = store.getRow("sessions", sessionId);
        if (!row) {
          return;
        }

        const event = getSessionEvent({
          event_json:
            typeof row.event_json === "string" ? row.event_json : undefined,
        });
        if (!event?.tracking_id) {
          return;
        }
        sessionByTrackingId.set(event.tracking_id, sessionId);
      });

      const allEvents = await getAllEvents();

      const rows: Array<{
        id: string;
        title: string;
        startedAt: string | null;
        endedAt: string | null;
        location: string | null;
        meetingLink: string | null;
        description: string | null;
        participantCount: number;
        linkedSessionId: string | null;
        startedAtMs: number;
      }> = [];

      for (const evt of allEvents) {
        const title = evt.title || "";
        const startedAt = evt.startedAt || null;
        const endedAt = evt.endedAt || null;
        const location = evt.location || null;
        const meetingLink = evt.meetingLink || null;
        const description = evt.description || null;
        const trackingId = evt.trackingIdEvent || "";

        let participantCount = 0;
        if (evt.participantsJson) {
          try {
            const raw =
              typeof evt.participantsJson === "string"
                ? evt.participantsJson
                : JSON.stringify(evt.participantsJson);
            const parsed = JSON.parse(raw);
            if (Array.isArray(parsed)) {
              participantCount = parsed.length;
            }
          } catch {}
        }

        const searchable = [title, location, meetingLink, description]
          .filter(Boolean)
          .join("\n")
          .toLowerCase();

        if (q && !searchable.includes(q)) {
          continue;
        }

        rows.push({
          id: evt.id,
          title: title || "Untitled event",
          startedAt,
          endedAt,
          location,
          meetingLink,
          description,
          participantCount,
          linkedSessionId: sessionByTrackingId.get(trackingId) ?? null,
          startedAtMs: startedAt ? Date.parse(startedAt) || 0 : 0,
        });
      }

      rows.sort((a, b) => b.startedAtMs - a.startedAtMs);

      return rows
        .slice(0, limit)
        .map(({ startedAtMs: _startedAtMs, ...row }) => row);
    },
    [store],
  );

  const { getSessionId, getEnhancedNoteId } = useSessionTab();
  const openEditTab = useCallback((requestId: string) => {
    useTabs.getState().openNew({ type: "edit", requestId });
  }, []);

  useRegisterTools(
    "chat-general",
    () =>
      buildChatTools({
        search,
        getContactSearchResults,
        getCalendarEventSearchResults,
        getStore: () => storeRef.current ?? undefined,
        getIndexes: () => indexesRef.current ?? undefined,
        getSessionId,
        getEnhancedNoteId,
        openEditTab,
      }),
    [
      search,
      getContactSearchResults,
      getCalendarEventSearchResults,
      getSessionId,
      getEnhancedNoteId,
      openEditTab,
    ],
  );

  return null;
}

function EnhancerInit() {
  const { persistedStore, aiTaskStore } = useRouteContext({
    from: "__root__",
  });

  const model = useLanguageModel("enhance");
  const { conn: llmConn } = useLLMConnection();
  const indexes = main.UI.useIndexes(main.STORE_ID);
  const selectedTemplateId = settings.UI.useValue(
    "selected_template_id",
    settings.STORE_ID,
  ) as string | undefined;

  const modelRef = useRef(model);
  modelRef.current = model;
  const llmConnRef = useRef(llmConn);
  llmConnRef.current = llmConn;
  const templateIdRef = useRef(selectedTemplateId);
  templateIdRef.current = selectedTemplateId;

  useEffect(() => {
    if (!persistedStore || !aiTaskStore || !indexes) return;

    const service = initEnhancerService({
      mainStore: persistedStore,
      indexes,
      aiTaskStore,
      getModel: () => modelRef.current,
      getLLMConn: () => llmConnRef.current,
      getSelectedTemplateId: () => templateIdRef.current || undefined,
    });

    return () => service.dispose();
  }, [persistedStore, aiTaskStore, indexes]);

  return null;
}
