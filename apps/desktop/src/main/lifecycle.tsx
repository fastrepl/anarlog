import { useRouteContext } from "@tanstack/react-router";
import { useCallback, useEffect, useRef } from "react";

import { useLanguageModel, useLLMConnection } from "~/ai/hooks";
import { useSessionTab } from "~/chat/components/use-session-tab";
import { buildChatTools } from "~/chat/tools";
import { useRegisterTools } from "~/contexts/tool";
import { useSearchEngine } from "~/search/contexts/engine";
import { initEnhancerService } from "~/services/enhancer";
import {
  useCalendarEventSearchIndex,
  useContactSearchIndex,
  useEnhancerSessionIndex,
  useSessionSearchTimestampLookup,
  useSessionTabLifecycle,
  useSummaryEditRuntime,
} from "~/session/hooks/storage";
import * as settings from "~/store/tinybase/store/settings";
import { useTabs } from "~/store/zustand/tabs";

export function useClassicMainLifecycle() {
  const openNew = useTabs((state) => state.openNew);

  const openDefaultEmptyTab = useCallback(() => {
    openNew({ type: "empty" });
  }, [openNew]);

  useSessionTabLifecycle({
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
  const getContactSearchResults = useContactSearchIndex();
  const getCalendarEventSearchResults = useCalendarEventSearchIndex();
  const getSessionSearchTimestamp = useSessionSearchTimestampLookup();
  const { getSummaryCandidates, getSummaryMarkdown, applySummaryMarkdown } =
    useSummaryEditRuntime();

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
        getSessionSearchTimestamp,
        getSummaryCandidates,
        getSummaryMarkdown,
        applySummaryMarkdown,
        getSessionId,
        getEnhancedNoteId,
        openEditTab,
      }),
    [
      search,
      getContactSearchResults,
      getCalendarEventSearchResults,
      getSessionSearchTimestamp,
      getSummaryCandidates,
      getSummaryMarkdown,
      applySummaryMarkdown,
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
  const childIndex = useEnhancerSessionIndex();
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
    if (!persistedStore || !aiTaskStore) return;

    const service = initEnhancerService({
      mainStore: persistedStore,
      childIndex,
      aiTaskStore,
      getModel: () => modelRef.current,
      getLLMConn: () => llmConnRef.current,
      getSelectedTemplateId: () => templateIdRef.current || undefined,
    });

    return () => service.dispose();
  }, [persistedStore, aiTaskStore, childIndex]);

  return null;
}
