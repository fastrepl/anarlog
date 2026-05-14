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
    createDefaultTemplate,
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

  const cloneAsMine = useCallback(
    async (
      draft: UserTemplateDraft,
      onCreate?: (id: string) => void | Promise<void>,
    ) => {
      const id = await createTemplate(draft);
      if (!id) return null;
      await onCreate?.(id);
      setSelectedMineId(id);
      return id;
    },
    [createTemplate, setSelectedMineId],
  );

  const handleCloneTemplate = useCallback(
    async (draft: UserTemplateDraft) => {
      await cloneAsMine({
        ...draft,
        title: getTemplateCopyTitle(draft.title),
      });
    },
    [cloneAsMine],
  );

  const handleFavoriteTemplate = useCallback(
    async (draft: UserTemplateDraft) => {
      await cloneAsMine(draft, (id) => toggleTemplateFavorite(id));
    },
    [cloneAsMine, toggleTemplateFavorite],
  );

  const handleSetDefaultTemplate = useCallback(
    async (draft: UserTemplateDraft) => {
      if (!settingsStore) return;
      const id = await cloneAsMine(draft);
      if (id) settingsStore.setValue("selected_template_id", id);
    },
    [cloneAsMine, settingsStore],
  );

  const handleDuplicateTemplate = useCallback(
    async (id: string) => {
      const template = userTemplates.find((t) => t.id === id);
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

  const selectedMineTemplate =
    userTemplates.find((template) => template.id === selectedMineId) ?? null;

  return (
    <div className="h-full">
      <TemplateDetailsColumn
        isWebMode={isWebMode}
        selectedMineTemplate={selectedMineTemplate}
        selectedWebTemplate={selectedWebTemplate}
        handleCreateTemplate={createDefaultTemplate}
        handleDeleteTemplate={handleDeleteTemplate}
        handleDuplicateTemplate={handleDuplicateTemplate}
        handleCloneTemplate={handleCloneTemplate}
        handleFavoriteTemplate={handleFavoriteTemplate}
        handleSetDefaultTemplate={handleSetDefaultTemplate}
      />
    </div>
  );
}
