import { MessageCircle } from "lucide-react";

import { useShell } from "~/contexts/shell";
import { type TabItem, TabItemBase } from "~/shared/tabs";
import type { Tab } from "~/store/zustand/tabs";

export const TabItemChat: TabItem<Extract<Tab, { type: "chat_support" }>> = ({
  tab,
  tabIndex,
  handleCloseThis,
  handleSelectThis,
  handleCloseOthers,
  handleCloseAll,
  handlePinThis,
  handleUnpinThis,
}) => {
  const { chat } = useShell();
  return (
    <TabItemBase
      icon={<MessageCircle className="h-4 w-4" />}
      title="Chat (Support)"
      selected={tab.active}
      pinned={tab.pinned}
      tabIndex={tabIndex}
      accent="blue"
      handleCloseThis={() => {
        chat.sendEvent({ type: "CLOSE" });
        handleCloseThis(tab);
      }}
      handleSelectThis={() => handleSelectThis(tab)}
      handleCloseOthers={handleCloseOthers}
      handleCloseAll={() => {
        chat.sendEvent({ type: "CLOSE" });
        handleCloseAll();
      }}
      handlePinThis={() => handlePinThis(tab)}
      handleUnpinThis={() => handleUnpinThis(tab)}
    />
  );
};
