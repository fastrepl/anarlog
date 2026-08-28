import * as stylex from "@stylexjs/stylex";

import { radii } from "@anlg/design-system/tokens.stylex";

import { ACCOUNT_TABS, type AccountTabId } from "@/lib/account-tabs";
const styles = stylex.create({
  tabList: {
    display: "flex",
    gap: ".25rem",
    overflowX: "auto",
  },
  tab: {
    borderRadius: radii.full,
    flexShrink: 0,
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    paddingBlock: ".375rem",
    paddingInline: ".75rem",
    transitionDuration: ".15s",
    transitionProperty: "color, background-color",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    whiteSpace: "nowrap",
  },
  activeTab: {
    backgroundColor: "#fff0b3",
    color: "#181613",
    fontWeight: 500,
  },
  inactiveTab: {
    color: {
      default: "#756b5d",
      ":hover": "#181613",
    },
  },
});
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
      <div role="tablist" {...stylex.props(styles.tabList)}>
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
              onPointerEnter={() => onPreload?.(tab.id)}
              onFocus={() => onPreload?.(tab.id)}
              onClick={() => onSelect(tab.id)}
              {...stylex.props([
                styles.tab,
                isActive ? styles.activeTab : styles.inactiveTab,
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
