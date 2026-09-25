import {
  Code,
  PlugsConnected,
  ShareNetwork,
  User,
  type Icon,
} from "@anlg/ui/components/icons";
import { cn } from "@anlg/utils";

import { ACCOUNT_TABS, type AccountTabId } from "@/lib/account-tabs";

const TAB_ICONS: Record<AccountTabId, Icon> = {
  account: User,
  connections: PlugsConnected,
  notes: ShareNetwork,
  developer: Code,
};

export function AccountTabs({
  activeId,
  onSelect,
  onPreload,
}: {
  activeId: AccountTabId;
  onSelect: (tabId: AccountTabId) => void;
  onPreload?: (tabId: AccountTabId) => void;
}) {
  return (
    <nav aria-label="Account sections">
      <div role="tablist" className="flex gap-1 overflow-x-auto">
        {ACCOUNT_TABS.map((tab) => {
          const isActive = tab.id === activeId;
          const TabIcon = TAB_ICONS[tab.id];

          return (
            <button
              key={tab.id}
              type="button"
              role="tab"
              id={`account-tab-${tab.id}`}
              aria-selected={isActive}
              aria-controls={`account-tabpanel-${tab.id}`}
              onPointerEnter={() => onPreload?.(tab.id)}
              onFocus={() => onPreload?.(tab.id)}
              onClick={() => onSelect(tab.id)}
              className={cn([
                "flex shrink-0 items-center gap-1.5 rounded-full px-3 py-1.5 text-sm whitespace-nowrap transition-colors",
                isActive
                  ? "bg-[#fff0b3] font-medium text-[#181613]"
                  : "text-[#756b5d] hover:text-[#181613]",
              ])}
            >
              <TabIcon size={16} aria-hidden="true" />
              {tab.label}
            </button>
          );
        })}
      </div>
    </nav>
  );
}
