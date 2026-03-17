import { UserIcon } from "lucide-react";

import { StandardTabWrapper } from "~/shared/main";
import { type TabItem, TabItemBase } from "~/shared/tabs";
import * as main from "~/store/tinybase/store/main";
import { type Tab } from "~/store/zustand/tabs";

export const TabItemHuman: TabItem<Extract<Tab, { type: "humans" }>> = ({
  tab,
  tabIndex,
  handleCloseThis,
  handleSelectThis,
  handleCloseOthers,
  handleCloseAll,
  handlePinThis,
  handleUnpinThis,
}) => {
  const title = main.UI.useCell("humans", tab.id, "name", main.STORE_ID);

  return (
    <TabItemBase
      icon={<UserIcon className="h-4 w-4" />}
      title={title ?? "Human"}
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

export function TabContentHuman({
  tab: _,
}: {
  tab: Extract<Tab, { type: "humans" }>;
}) {
  return (
    <StandardTabWrapper>
      <div>Human</div>
    </StandardTabWrapper>
  );
}
