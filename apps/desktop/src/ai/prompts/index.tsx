import { SparklesIcon } from "lucide-react";
import { useCallback } from "react";

import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@hypr/ui/components/ui/resizable";

import { PromptDetailsColumn } from "./details";
import { PromptsListColumn } from "./list";

import { StandardTabWrapper } from "~/shared/main";
import { type TabItem, TabItemBase } from "~/shared/tabs";
import type { TaskType } from "~/store/tinybase/store/prompts";
import { type Tab, useTabs } from "~/store/zustand/tabs";

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
      <PromptView tab={tab} />
    </StandardTabWrapper>
  );
}

function PromptView({ tab }: { tab: Extract<Tab, { type: "prompts" }> }) {
  const updatePromptsTabState = useTabs((state) => state.updatePromptsTabState);

  const { selectedTask } = tab.state;

  const setSelectedTask = useCallback(
    (value: string | null) => {
      updatePromptsTabState(tab, {
        ...tab.state,
        selectedTask: value,
      });
    },
    [updatePromptsTabState, tab],
  );

  return (
    <ResizablePanelGroup direction="horizontal" className="h-full">
      <ResizablePanel defaultSize={30} minSize={20} maxSize={40}>
        <PromptsListColumn
          selectedTask={selectedTask as TaskType | null}
          setSelectedTask={setSelectedTask}
        />
      </ResizablePanel>
      <ResizableHandle />
      <ResizablePanel defaultSize={70} minSize={50}>
        <PromptDetailsColumn selectedTask={selectedTask as TaskType | null} />
      </ResizablePanel>
    </ResizablePanelGroup>
  );
}
