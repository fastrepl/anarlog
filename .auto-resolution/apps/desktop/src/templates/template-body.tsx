import { useCallback } from "react";

import { TemplateDetailsColumn } from "./details";
import { getTemplateCopyTitle, type UserTemplateDraft } from "./queries";
import { AUTO_TEMPLATE_ID, useTemplateTab } from "./utils";

import { setSettingValue } from "~/settings/queries";
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
      const id = await cloneAsMine(draft);
      if (id) await setSettingValue("selected_template_id", id);
    },
    [cloneAsMine],
  );

  const handleDuplicateTemplate = useCallback(
    async (id: string) => {
      const template = userTemplates.find((t) => t.id === id);
      if (!template) return;
      await handleCloneTemplate({
        title: template.title,
        description: template.description,
        category: template.category,
        icon: template.icon,
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
        isAutoSelected={!isWebMode && selectedMineId === AUTO_TEMPLATE_ID}
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
