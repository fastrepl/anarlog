import { useCallback } from "react";

import { TemplateDetailsColumn } from "./details";
import { getTemplateCopyTitle, type UserTemplateDraft } from "./queries";
import { useTemplateTab } from "./utils";

import * as settings from "~/store/tinybase/store/settings";
import { type Tab } from "~/store/zustand/tabs";

export function TemplateView({
  tab,
}: {
  tab: Extract<Tab, { type: "templates" }>;
}) {
  const {
    userTemplates,
    isWebMode,
    selectedMineId,
    selectedWebTemplate,
    setSelectedMineId,
    createTemplate,
    deleteTemplate,
    toggleTemplateFavorite,
  } = useTemplateTab(tab);
  const settingsStore = settings.UI.useStore(settings.STORE_ID);

  const handleDeleteTemplate = useCallback(
    async (id: string) => {
      await deleteTemplate(id);
      setSelectedMineId(null);
    },
    [deleteTemplate, setSelectedMineId],
  );

  const materializeTemplate = useCallback(
    async (
      template: UserTemplateDraft,
      {
        title = template.title,
        onCreate,
      }: {
        title?: string;
        onCreate?: (id: string) => void | Promise<void>;
      } = {},
    ) => {
      const id = await createTemplate({
        ...template,
        title,
      });
      if (!id) {
        return null;
      }

      await onCreate?.(id);
      setSelectedMineId(id);
      return id;
    },
    [createTemplate, setSelectedMineId],
  );

  const handleCloneTemplate = useCallback(
    async (template: UserTemplateDraft) => {
      await materializeTemplate(template, {
        title: getTemplateCopyTitle(template.title),
      });
    },
    [materializeTemplate],
  );

  const handleFavoriteTemplate = useCallback(
    async (template: UserTemplateDraft) => {
      await materializeTemplate(template, {
        onCreate: async (id) => {
          await toggleTemplateFavorite(id);
        },
      });
    },
    [materializeTemplate, toggleTemplateFavorite],
  );

  const handleSetDefaultTemplate = useCallback(
    async (template: UserTemplateDraft) => {
      if (!settingsStore) {
        return;
      }

      const id = await materializeTemplate(template);
      if (!id) {
        return;
      }

      settingsStore.setValue("selected_template_id", id);
    },
    [materializeTemplate, settingsStore],
  );

  const handleDuplicateTemplate = useCallback(
    async (id: string) => {
      const template = userTemplates.find((item) => item.id === id);
      if (!template) return;

      await handleCloneTemplate({
        title: template.title,
        description: template.description,
        category: template.category,
        targets: template.targets,
        sections: template.sections,
      });
    },
    [handleCloneTemplate, userTemplates],
  );

  return (
    <div className="h-full">
      <TemplateDetailsColumn
        isWebMode={isWebMode}
        selectedMineId={selectedMineId}
        selectedWebTemplate={selectedWebTemplate}
        handleDeleteTemplate={handleDeleteTemplate}
        handleDuplicateTemplate={handleDuplicateTemplate}
        handleCloneTemplate={handleCloneTemplate}
        handleFavoriteTemplate={handleFavoriteTemplate}
        handleSetDefaultTemplate={handleSetDefaultTemplate}
      />
    </div>
  );
}
