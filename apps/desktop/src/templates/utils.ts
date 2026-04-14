import { useCallback, useMemo } from "react";

import { parseWebTemplates, type WebTemplate } from "./codec";
import {
  useCreateTemplate,
  useDeleteTemplate,
  useToggleTemplateFavorite,
  useUserTemplates,
  type UserTemplate,
} from "./queries";

import { useWebResources } from "~/shared/ui/resource-list";
import * as main from "~/store/tinybase/store/main";
import { type Tab, useTabs } from "~/store/zustand/tabs";

export function resolveTemplateTabSelection({
  isWebMode,
  selectedMineId,
  selectedWebIndex,
  userTemplates,
  webTemplates,
}: {
  isWebMode: boolean | null | undefined;
  selectedMineId: string | null | undefined;
  selectedWebIndex: number | null | undefined;
  userTemplates: UserTemplate[];
  webTemplates: WebTemplate[];
}) {
  const hasUserTemplates = userTemplates.length > 0;
  const hasWebTemplates = webTemplates.length > 0;

  let effectiveIsWebMode = isWebMode ?? !hasUserTemplates;

  if (effectiveIsWebMode && !hasWebTemplates && hasUserTemplates) {
    effectiveIsWebMode = false;
  }

  if (!effectiveIsWebMode && !hasUserTemplates && hasWebTemplates) {
    effectiveIsWebMode = true;
  }

  if (effectiveIsWebMode) {
    const effectiveSelectedWebIndex =
      selectedWebIndex !== null &&
      selectedWebIndex !== undefined &&
      selectedWebIndex >= 0 &&
      selectedWebIndex < webTemplates.length
        ? selectedWebIndex
        : hasWebTemplates
          ? 0
          : null;

    return {
      isWebMode: true,
      selectedMineId: null,
      selectedWebIndex: effectiveSelectedWebIndex,
      selectedWebTemplate:
        effectiveSelectedWebIndex !== null
          ? (webTemplates[effectiveSelectedWebIndex] ?? null)
          : null,
    };
  }

  return {
    isWebMode: false,
    selectedMineId:
      userTemplates.find((template) => template.id === selectedMineId)?.id ??
      userTemplates[0]?.id ??
      null,
    selectedWebIndex: null,
    selectedWebTemplate: null,
  };
}

export function useTemplateCreatorName() {
  const userId = main.UI.useValue("user_id", main.STORE_ID);
  const name = main.UI.useCell("humans", userId ?? "", "name", main.STORE_ID);

  return typeof name === "string" && name.trim().length > 0
    ? name.trim()
    : "user";
}

export function getTemplateCreatorLabel({
  isUserTemplate,
  creatorName,
  format = "full",
}: {
  isUserTemplate: boolean;
  creatorName?: string | null;
  format?: "full" | "short";
}) {
  const name = isUserTemplate ? creatorName?.trim() || "user" : "Char";
  return format === "short" ? `by ${name}` : `Created by ${name}`;
}

export function useTemplateTab(tab: Extract<Tab, { type: "templates" }>) {
  const updateTabState = useTabs((state) => state.updateTemplatesTabState);
  const userTemplates = useUserTemplates();
  const createTemplate = useCreateTemplate();
  const deleteTemplate = useDeleteTemplate();
  const toggleTemplateFavorite = useToggleTemplateFavorite();
  const { data: rawWebTemplates = [], isLoading: isWebLoading } =
    useWebResources<Record<string, unknown>>("templates");
  const webTemplates = useMemo(
    () => parseWebTemplates(rawWebTemplates),
    [rawWebTemplates],
  );

  const { isWebMode, selectedMineId, selectedWebIndex, selectedWebTemplate } =
    resolveTemplateTabSelection({
      isWebMode: tab.state.isWebMode,
      selectedMineId: tab.state.selectedMineId,
      selectedWebIndex: tab.state.selectedWebIndex,
      userTemplates,
      webTemplates,
    });

  const setSelectedMineId = useCallback(
    (id: string | null) => {
      updateTabState(tab, {
        ...tab.state,
        isWebMode: false,
        selectedMineId: id,
        selectedWebIndex: null,
      });
    },
    [updateTabState, tab],
  );

  const setSelectedWebIndex = useCallback(
    (index: number | null) => {
      updateTabState(tab, {
        ...tab.state,
        isWebMode: true,
        selectedMineId: null,
        selectedWebIndex: index,
      });
    },
    [updateTabState, tab],
  );

  return {
    userTemplates,
    webTemplates,
    isWebLoading,
    isWebMode,
    selectedMineId,
    selectedWebIndex,
    selectedWebTemplate,
    setSelectedMineId,
    setSelectedWebIndex,
    createTemplate,
    deleteTemplate,
    toggleTemplateFavorite,
  };
}
