import { useCallback, useEffect, useRef } from "react";

import { type Tab, uniqueIdfromTab } from "~/store/zustand/tabs";

export function useScrollActiveTabIntoView(tabs: Tab[]) {
  const tabRefsMap = useRef<Map<string, HTMLDivElement>>(new Map());
  const activeTab = tabs.find((tab) => tab.active);
  const activeTabKey = activeTab ? uniqueIdfromTab(activeTab) : null;

  useEffect(() => {
    if (!activeTabKey) {
      return;
    }

    const tabElement = tabRefsMap.current.get(activeTabKey);
    if (!tabElement) {
      return;
    }

    tabElement.scrollIntoView({
      behavior: "smooth",
      inline: "nearest",
      block: "nearest",
    });
  }, [activeTabKey]);

  return useCallback((tab: Tab, element: HTMLDivElement | null) => {
    const key = uniqueIdfromTab(tab);
    if (element) {
      tabRefsMap.current.set(key, element);
    } else {
      tabRefsMap.current.delete(key);
    }
  }, []);
}
