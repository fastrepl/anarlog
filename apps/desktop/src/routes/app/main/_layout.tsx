import {
  createFileRoute,
  Outlet,
  useRouteContext,
} from "@tanstack/react-router";
import { isTauri } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef } from "react";

import { AITaskProvider } from "~/ai/contexts";
import { useLanguageModel, useLLMConnection } from "~/ai/hooks";
import { useSessionTab } from "~/chat/components/use-session-tab";
import { buildChatTools } from "~/chat/tools";
import { NotificationProvider } from "~/contexts/notifications";
import { ShellProvider } from "~/contexts/shell";
import { useRegisterTools } from "~/contexts/tool";
import { ToolRegistryProvider } from "~/contexts/tool";
import { useSearchEngine } from "~/search/contexts/engine";
import { SearchEngineProvider } from "~/search/contexts/engine";
import { SearchUIProvider } from "~/search/contexts/ui";
import { initEnhancerService } from "~/services/enhancer";
import { getSessionEvent } from "~/session/utils";
import { deleteSessionCascade } from "~/store/tinybase/store/deleteSession";
import * as main from "~/store/tinybase/store/main";
import { isSessionEmpty } from "~/store/tinybase/store/sessions";
import * as settings from "~/store/tinybase/store/settings";
import { listenerStore } from "~/store/zustand/listener/instance";
import {
  restorePinnedTabsToStore,
  restoreRecentlyOpenedToStore,
  useTabs,
} from "~/store/zustand/tabs";
import { commands } from "~/types/tauri.gen";

export const Route = createFileRoute("/app/main/_layout")({
  component: Component,
});

function Component() {
  const { persistedStore, aiTaskStore, toolRegistry } = useRouteContext({
    from: "__root__",
  });
  const {
    registerOnEmpty,
    registerCanClose,
    registerOnClose,
    openNew,
    pin,
    invalidateResource,
  } = useTabs();
  const hasOpenedInitialTab = useRef(false);
  const store = main.UI.useStore(main.STORE_ID);
  const indexes = main.UI.useIndexes(main.STORE_ID);

  const openDefaultEmptyTab = useCallback(() => {
    openNew({ type: "empty" });
  }, [openNew]);

  useEffect(() => {
    const initializeTabs = async () => {
      if (!hasOpenedInitialTab.current) {
        hasOpenedInitialTab.current = true;
        if (!isTauri()) {
          openDefaultEmptyTab();
          return;
        }
        await restorePinnedTabsToStore(
          openNew,
          pin,
          () => useTabs.getState().tabs,
        );
        await restoreRecentlyOpenedToStore((ids) => {
          useTabs.setState({ recentlyOpenedSessionIds: ids });
        });
        const currentTabs = useTabs.getState().tabs;
        if (currentTabs.length === 0) {
          const result = await commands.getOnboardingNeeded();
          if (result.status === "ok" && result.data) {
            openNew({ type: "onboarding" });
          } else {
            openDefaultEmptyTab();
          }
        }
      }
    };

    initializeTabs();
    registerOnEmpty(openDefaultEmptyTab);
  }, [openNew, pin, openDefaultEmptyTab, registerOnEmpty]);

  useEffect(() => {
    registerCanClose(() => true);
  }, [registerCanClose]);

  useEffect(() => {
    if (!store) {
      return;
    }
    registerOnClose((tab) => {
      if (tab.type === "sessions") {
        const sessionId = tab.id;
        const isBatchRunning =
          listenerStore.getState().getSessionMode(sessionId) ===
          "running_batch";
        if (!isBatchRunning && isSessionEmpty(store, sessionId)) {
          invalidateResource("sessions", sessionId);
          void deleteSessionCascade(store, indexes, sessionId);
        }
      }
    });
  }, [registerOnClose, invalidateResource, store, indexes]);

  if (!aiTaskStore) {
    return null;
  }

  return (
    <SearchEngineProvider store={persistedStore}>
      <SearchUIProvider>
        <ShellProvider>
          <ToolRegistryProvider registry={toolRegistry}>
            <AITaskProvider store={aiTaskStore}>
              <NotificationProvider>
                <ToolRegistration />
                <EnhancerInit />
                <Outlet />
              </NotificationProvider>
            </AITaskProvider>
          </ToolRegistryProvider>
        </ShellProvider>
      </SearchUIProvider>
    </SearchEngineProvider>
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

      store.forEachRow("events", (eventId, _forEachCell) => {
        const row = store.getRow("events", eventId);
        if (!row) {
          return;
        }

        const title = typeof row.title === "string" ? row.title : "";
        const startedAt =
          typeof row.started_at === "string" && row.started_at
            ? row.started_at
            : null;
        const endedAt =
          typeof row.ended_at === "string" && row.ended_at
            ? row.ended_at
            : null;
        const location =
          typeof row.location === "string" && row.location
            ? row.location
            : null;
        const meetingLink =
          typeof row.meeting_link === "string" && row.meeting_link
            ? row.meeting_link
            : null;
        const description =
          typeof row.description === "string" && row.description
            ? row.description
            : null;
        const trackingId =
          typeof row.tracking_id_event === "string"
            ? row.tracking_id_event
            : "";

        let participantCount = 0;
        if (
          typeof row.participants_json === "string" &&
          row.participants_json
        ) {
          try {
            const parsed = JSON.parse(row.participants_json);
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
          return;
        }

        rows.push({
          id: eventId,
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
      });

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
