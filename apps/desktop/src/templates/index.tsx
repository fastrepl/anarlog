import { BookText } from "lucide-react";
import { useCallback, useMemo } from "react";

import type { TemplateSection } from "@hypr/store";

import { TemplateDetailsColumn } from "./components/details";
import {
  parseWebTemplates,
  resolveTemplateTabSelection,
  useCreateTemplate,
  useDeleteTemplate,
  useToggleTemplateFavorite,
  useUserTemplates,
} from "./shared";

import { StandardTabWrapper } from "~/shared/main";
import { type TabItem, TabItemBase } from "~/shared/tabs";
import { useWebResources } from "~/shared/ui/resource-list";
import * as settings from "~/store/tinybase/store/settings";
import { type Tab, useTabs } from "~/store/zustand/tabs";

export {
  getTemplateCreatorLabel,
  parseWebTemplates,
  useCreateTemplate,
  useTemplateCreatorName,
  useUserTemplate,
  useUserTemplates,
} from "./shared";

export const TabItemTemplate: TabItem<Extract<Tab, { type: "templates" }>> = ({
  tab,
  tabIndex,
  handleCloseThis,
  handleSelectThis,
  handleCloseOthers,
  handleCloseAll,
  handlePinThis,
  handleUnpinThis,
}) => {
  return (
    <TabItemBase
      icon={<BookTextIcon className="h-4 w-4" />}
      title={"Templates"}
      selected={tab.active}
      pinned={tab.pinned}
      tabIndex={tabIndex}
      handleCloseThis={() => handleCloseThis(tab)}
      handleSelectThis={() => handleSelectThis(tab)}
      handleCloseOthers={handleCloseOthers}
      handleCloseAll={handleCloseAll}
      handlePinThis={() => handlePinThis(tab)}
      handleUnpinThis={() => handleUnpinThis(tab)}
    />
  );
};

function BookTextIcon({ className }: { className?: string }) {
  return <BookText className={className} />;
}

export function TabContentTemplate({
  tab,
}: {
  tab: Extract<Tab, { type: "templates" }>;
}) {
  return (
    <StandardTabWrapper>
      <TemplateView tab={tab} />
    </StandardTabWrapper>
  );
}

function TemplateView({ tab }: { tab: Extract<Tab, { type: "templates" }> }) {
  const updateTabState = useTabs((state) => state.updateTemplatesTabState);
  const userTemplates = useUserTemplates();
  const createTemplate = useCreateTemplate();
  const deleteTemplate = useDeleteTemplate();
  const toggleTemplateFavorite = useToggleTemplateFavorite();
  const { data: rawWebTemplates = [] } =
    useWebResources<Record<string, unknown>>("templates");
  const webTemplates = useMemo(
    () => parseWebTemplates(rawWebTemplates),
    [rawWebTemplates],
  );
  const settingsStore = settings.UI.useStore(settings.STORE_ID);

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

  const { isWebMode, selectedMineId, selectedWebTemplate } =
    resolveTemplateTabSelection({
      isWebMode: tab.state.isWebMode,
      selectedMineId: tab.state.selectedMineId,
      selectedWebIndex: tab.state.selectedWebIndex,
      userTemplates,
      webTemplates,
    });

  const handleDeleteTemplate = useCallback(
    async (id: string) => {
      await deleteTemplate(id);
      setSelectedMineId(null);
    },
    [deleteTemplate, setSelectedMineId],
  );

  const materializeTemplate = useCallback(
    async (
      template: {
        title: string;
        description: string;
        category?: string;
        targets?: string[];
        sections: TemplateSection[];
      },
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
    async (template: {
      title: string;
      description: string;
      category?: string;
      targets?: string[];
      sections: TemplateSection[];
    }) => {
      await materializeTemplate(template, {
        title: getTemplateCopyTitle(template.title),
      });
    },
    [materializeTemplate],
  );

  const handleFavoriteTemplate = useCallback(
    async (template: {
      title: string;
      description: string;
      category?: string;
      targets?: string[];
      sections: TemplateSection[];
    }) => {
      await materializeTemplate(template, {
        onCreate: async (id) => {
          await toggleTemplateFavorite(id);
        },
      });
    },
    [materializeTemplate, toggleTemplateFavorite],
  );

  const handleSetDefaultTemplate = useCallback(
    async (template: {
      title: string;
      description: string;
      category?: string;
      targets?: string[];
      sections: TemplateSection[];
    }) => {
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

function getTemplateCopyTitle(title: string) {
  const value = title.trim();

  if (!value) return "Untitled (Copy)";
  if (value.endsWith("(Copy)")) return value;

  return `${value} (Copy)`;
}
