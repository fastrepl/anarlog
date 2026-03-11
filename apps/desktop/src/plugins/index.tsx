import { PuzzleIcon } from "lucide-react";

import { getPluginDisplayName, getPluginView } from "./registry";

import { StandardTabWrapper } from "~/shared/main";
import { type TabItem, TabItemBase } from "~/shared/tabs";
import { type Tab } from "~/store/zustand/tabs";

type PluginTab = Extract<Tab, { type: "extension" }>;

export const TabItemPlugin: TabItem<PluginTab> = ({
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
      icon={<PuzzleIcon className="h-4 w-4" />}
      title={getPluginDisplayName(tab.extensionId)}
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

export function TabContentPlugin({ tab }: { tab: PluginTab }) {
  const render = getPluginView(tab.extensionId);

  return (
    <StandardTabWrapper>
      {render?.() ?? (
        <div className="flex h-full items-center justify-center">
          <div className="text-center">
            <PuzzleIcon size={48} className="mx-auto mb-4 text-neutral-300" />
            <p className="text-neutral-500">
              Plugin not found: {tab.extensionId}
            </p>
          </div>
        </div>
      )}
    </StandardTabWrapper>
  );
}
