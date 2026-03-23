import { PencilIcon } from "lucide-react";

import { TabItemBase, type TabItem } from "~/shared/tabs";
import { type Tab } from "~/store/zustand/tabs";

type EditTab = Extract<Tab, { type: "edit" }>;

export const TabItemEdit: TabItem<EditTab> = ({
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
      icon={<PencilIcon className="h-4 w-4" />}
      title="Review Edit"
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
