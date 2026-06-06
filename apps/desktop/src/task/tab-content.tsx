import { useCallback, useRef, useState } from "react";

import { cn } from "@hypr/utils";

import { ResourceView } from "./resource-view";

import { ChatCTA } from "~/shared/chat-cta";
import { StandardTabWrapper } from "~/shared/main";
import { type Tab, type TaskResource } from "~/store/zustand/tabs";

type TaskTab = Extract<Tab, { type: "task" }>;

export function TabContentTask({ tab }: { tab: TaskTab }) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const sectionRefs = useRef<Map<string, HTMLDivElement>>(new Map());
  const [activeKey, setActiveKey] = useState<string | null>(null);

  const handleNavClick = useCallback((key: string) => {
    const element = sectionRefs.current.get(key);
    if (element) {
      element.scrollIntoView({ behavior: "smooth", block: "start" });
    }
  }, []);

  const handleScroll = useCallback(() => {
    const container = scrollRef.current;
    if (!container) return;

    let closest: string | null = null;
    let closestDistance = Infinity;

    for (const [key, element] of sectionRefs.current.entries()) {
      const rect = element.getBoundingClientRect();
      const containerRect = container.getBoundingClientRect();
      const distance = Math.abs(rect.top - containerRect.top);
      if (distance < closestDistance) {
        closestDistance = distance;
        closest = key;
      }
    }

    setActiveKey(closest);
  }, []);

  const registerRef = useCallback(
    (key: string, element: HTMLDivElement | null) => {
      if (element) {
        sectionRefs.current.set(key, element);
      } else {
        sectionRefs.current.delete(key);
      }
    },
    [],
  );

  const floatingButton = (
    <div className="absolute bottom-4 left-1/2 z-20 -translate-x-1/2">
      <ChatCTA label="Work on this task" />
    </div>
  );

  const showNav = tab.resources.length > 1;

  return (
    <StandardTabWrapper floatingButton={floatingButton}>
      <div
        ref={scrollRef}
        className="relative h-full overflow-auto"
        onScroll={handleScroll}
      >
        <div className="flex">
          <div className="min-w-0 flex-1">
            {tab.resources.map((resource, index) => {
              const key = resourceKey(resource);
              return (
                <div key={key}>
                  {index > 0 ? (
                    <div className="max-w-3xl px-6">
                      <div className="border-border border-t-2" />
                    </div>
                  ) : null}
                  <div ref={(element) => registerRef(key, element)}>
                    <ResourceView resource={resource} />
                  </div>
                </div>
              );
            })}
            <div className="h-20" />
          </div>
          {showNav ? (
            <ResourceNav
              resources={tab.resources}
              activeKey={activeKey}
              onNavClick={handleNavClick}
            />
          ) : null}
        </div>
      </div>
    </StandardTabWrapper>
  );
}

function ResourceNav({
  resources,
  activeKey,
  onNavClick,
}: {
  resources: TaskResource[];
  activeKey: string | null;
  onNavClick: (key: string) => void;
}) {
  return (
    <div className="sticky top-0 flex w-40 shrink-0 flex-col justify-center self-start px-2 py-6">
      <div className="space-y-0.5">
        {resources.map((resource) => {
          const key = resourceKey(resource);
          const isActive = activeKey === key;
          return (
            <button
              key={key}
              type="button"
              onClick={() => onNavClick(key)}
              className={cn([
                "w-full rounded-md px-2 py-1.5 text-left text-xs transition-colors",
                isActive
                  ? "text-foreground font-medium"
                  : "text-muted-foreground hover:text-muted-foreground",
              ])}
            >
              <span className="line-clamp-2">
                {resource.owner}/{resource.repo} #{resource.number}
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
}

function resourceKey(resource: TaskResource): string {
  return `${resource.type}-${resource.owner}-${resource.repo}-${resource.number}`;
}
