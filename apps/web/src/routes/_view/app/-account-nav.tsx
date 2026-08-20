import { cn } from "@anlg/utils";

import { ACCOUNT_TABS, type AccountTabId } from "@/lib/account-tabs";

export function AccountTabs({
  activeId,
  onSelect,
}: {
  activeId: AccountTabId;
  onSelect: (tabId: AccountTabId) => void;
}) {
  return (
    <nav aria-label="Account sections">
      <div role="tablist" className="flex gap-1 overflow-x-auto">
        {ACCOUNT_TABS.map((tab) => {
          const isActive = tab.id === activeId;

          return (
            <button
              key={tab.id}
              type="button"
              role="tab"
              id={`account-tab-${tab.id}`}
              aria-selected={isActive}
              aria-controls={`account-tabpanel-${tab.id}`}
              onClick={() => onSelect(tab.id)}
              className={cn([
                "shrink-0 rounded-full px-3 py-1.5 text-sm whitespace-nowrap transition-colors",
                isActive
                  ? "bg-[#fff0b3] font-medium text-[#181613]"
                  : "text-[#756b5d] hover:text-[#181613]",
              ])}
            >
              {tab.label}
            </button>
          );
        })}
      </div>
    </nav>
  );
}
