import { BookText } from "lucide-react";
import { useCallback } from "react";

import type { TemplateSection } from "@hypr/store";

import { TemplateDetailsColumn } from "./components/details";
import {
  resolveTemplateTabSelection,
  useCreateTemplate,
  useUserTemplates,
  type WebTemplate,
} from "./shared";

import { StandardTabWrapper } from "~/shared/main";
import { type TabItem, TabItemBase } from "~/shared/tabs";
import { useWebResources } from "~/shared/ui/resource-list";
import * as main from "~/store/tinybase/store/main";
import { type Tab, useTabs } from "~/store/zustand/tabs";

export { useUserTemplates } from "./shared";

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
  const { data: webTemplates = [] } = useWebResources<WebTemplate>("templates");

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

  const deleteTemplateFromStore = main.UI.useDelRowCallback(
    "templates",
    (templateId: string) => templateId,
    main.STORE_ID,
  );

  const handleDeleteTemplate = useCallback(
    (id: string) => {
      deleteTemplateFromStore(id);
      setSelectedMineId(null);
    },
    [deleteTemplateFromStore, setSelectedMineId],
  );

  const handleCloneTemplate = useCallback(
    (template: {
      title: string;
      description: string;
      sections: TemplateSection[];
    }) => {
      const id = createTemplate(template);
      if (id) {
        setSelectedMineId(id);
      }
    },
    [createTemplate, setSelectedMineId],
  );

  return (
    <div className="h-full">
      <TemplateDetailsColumn
        isWebMode={isWebMode}
        selectedMineId={selectedMineId}
        selectedWebTemplate={selectedWebTemplate}
        handleDeleteTemplate={handleDeleteTemplate}
        handleCloneTemplate={handleCloneTemplate}
      />
    </div>
  );
}
