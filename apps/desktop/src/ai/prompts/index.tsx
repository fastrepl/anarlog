import { SparklesIcon } from "lucide-react";

import { PromptDetailsColumn } from "./details";

import type { TaskType } from "~/ai/prompts/config";
import { StandardTabWrapper } from "~/shared/main";
import { type TabItem, TabItemBase } from "~/shared/tabs";
import { type Tab } from "~/store/zustand/tabs";

export const TabItemPrompt: TabItem<Extract<Tab, { type: "prompts" }>> = ({
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
      icon={<SparklesIcon className="h-4 w-4" />}
      title={"Prompts"}
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

export function TabContentPrompt({
  tab,
}: {
  tab: Extract<Tab, { type: "prompts" }>;
}) {
  return (
    <StandardTabWrapper>
      <PromptDetailsColumn
        selectedTask={tab.state.selectedTask as TaskType | null}
      />
    </StandardTabWrapper>
  );
}
