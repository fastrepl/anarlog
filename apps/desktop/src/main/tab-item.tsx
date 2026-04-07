import { TabItemEmpty } from "./empty";

import { MainTabItem } from "~/shared/main";
import { type Tab } from "~/store/zustand/tabs";

export function ClassicMainTabItem({
  tab,
  handleClose,
  handleSelect,
  handleCloseOthersCallback,
  handleCloseAll,
  handlePin,
  handleUnpin,
  tabIndex,
  pendingCloseConfirmationTab,
  setPendingCloseConfirmationTab,
}: {
  tab: Tab;
  handleClose: (tab: Tab) => void;
  handleSelect: (tab: Tab) => void;
  handleCloseOthersCallback: (tab: Tab) => void;
  handleCloseAll: () => void;
  handlePin: (tab: Tab) => void;
  handleUnpin: (tab: Tab) => void;
  tabIndex?: number;
  pendingCloseConfirmationTab?: Tab | null;
  setPendingCloseConfirmationTab?: (tab: Tab | null) => void;
}) {
  if (tab.type === "empty") {
    return (
      <TabItemEmpty
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={() => handleCloseOthersCallback(tab)}
        handleCloseAll={handleCloseAll}
        handlePinThis={() => handlePin(tab)}
        handleUnpinThis={() => handleUnpin(tab)}
      />
    );
  }

  return (
    <MainTabItem
      tab={tab}
      handleClose={handleClose}
      handleSelect={handleSelect}
      handleCloseOthersCallback={handleCloseOthersCallback}
      handleCloseAll={handleCloseAll}
      handlePin={handlePin}
      handleUnpin={handleUnpin}
      tabIndex={tabIndex}
      pendingCloseConfirmationTab={pendingCloseConfirmationTab}
      setPendingCloseConfirmationTab={setPendingCloseConfirmationTab}
    />
  );
}
