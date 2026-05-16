import { TabItemCalendar } from "~/calendar";
import { TabItemChangelog } from "~/changelog";
import { TabItemContact } from "~/contacts";
import { TabItemHuman } from "~/contacts/humans";
import { TabItemEdit } from "~/edit";
import { TabItemOnboarding } from "~/onboarding";
import { TabItemNote } from "~/session";
import { TabItemSettings } from "~/settings";
import { type Tab } from "~/store/zustand/tabs";
import { TabItemTask } from "~/task";
import { TabItemTemplate } from "~/templates";

export function MainTabItem({
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
  const handleCloseOthers = () => handleCloseOthersCallback(tab);
  const handlePinThis = () => handlePin(tab);
  const handleUnpinThis = () => handleUnpin(tab);

  if (tab.type === "sessions") {
    return (
      <TabItemNote
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={handlePinThis}
        handleUnpinThis={handleUnpinThis}
        pendingCloseConfirmationTab={pendingCloseConfirmationTab}
        setPendingCloseConfirmationTab={setPendingCloseConfirmationTab}
      />
    );
  }
  if (tab.type === "humans") {
    return (
      <TabItemHuman
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={handlePinThis}
        handleUnpinThis={handleUnpinThis}
      />
    );
  }
  if (tab.type === "contacts") {
    return (
      <TabItemContact
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={handlePinThis}
        handleUnpinThis={handleUnpinThis}
      />
    );
  }
  if (tab.type === "calendar") {
    return (
      <TabItemCalendar
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={handlePinThis}
        handleUnpinThis={handleUnpinThis}
      />
    );
  }
  if (tab.type === "changelog") {
    return (
      <TabItemChangelog
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={handlePinThis}
        handleUnpinThis={handleUnpinThis}
      />
    );
  }
  if (tab.type === "settings") {
    return (
      <TabItemSettings
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={handlePinThis}
        handleUnpinThis={handleUnpinThis}
      />
    );
  }
  if (tab.type === "templates") {
    return (
      <TabItemTemplate
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={handlePinThis}
        handleUnpinThis={handleUnpinThis}
      />
    );
  }
  if (tab.type === "onboarding") {
    return (
      <TabItemOnboarding
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={handlePinThis}
        handleUnpinThis={handleUnpinThis}
      />
    );
  }
  if (tab.type === "edit") {
    return (
      <TabItemEdit
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={handlePinThis}
        handleUnpinThis={handleUnpinThis}
      />
    );
  }
  if (tab.type === "task") {
    return (
      <TabItemTask
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={handlePinThis}
        handleUnpinThis={handleUnpinThis}
      />
    );
  }
  return null;
}
