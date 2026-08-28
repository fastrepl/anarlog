import { Trans } from "@lingui/react/macro";
import * as stylex from "@stylexjs/stylex";
import { useCallback, useRef, useState } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";

import { ResourceView } from "./resource-view";

import { ChatCTA } from "~/shared/chat-cta";
import { StandardContentWrapper } from "~/shared/main";
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
    <div {...stylex.props(styles.floatingButton)}>
      <ChatCTA label={<Trans>Work on this task</Trans>} />
    </div>
  );

  const showNav = tab.resources.length > 1;

  return (
    <StandardContentWrapper floatingButton={floatingButton}>
      <div
        ref={scrollRef}
        {...stylex.props(styles.scrollContainer)}
        onScroll={handleScroll}
      >
        <div {...stylex.props(styles.contentLayout)}>
          <div {...stylex.props(styles.resources)}>
            {tab.resources.map((resource, index) => {
              const key = resourceKey(resource);
              return (
                <div key={key}>
                  {index > 0 ? (
                    <div {...stylex.props(styles.dividerContainer)}>
                      <div {...stylex.props(styles.divider)} />
                    </div>
                  ) : null}
                  <div ref={(element) => registerRef(key, element)}>
                    <ResourceView resource={resource} />
                  </div>
                </div>
              );
            })}
            <div {...stylex.props(styles.bottomSpacer)} />
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
    </StandardContentWrapper>
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
    <div {...stylex.props(styles.navigation)}>
      <div {...stylex.props(styles.navigationList)}>
        {resources.map((resource) => {
          const key = resourceKey(resource);
          const isActive = activeKey === key;
          return (
            <button
              key={key}
              type="button"
              onClick={() => onNavClick(key)}
              {...stylex.props(
                styles.navigationButton,
                isActive
                  ? styles.navigationButtonActive
                  : styles.navigationButtonInactive,
              )}
            >
              <span {...stylex.props(styles.navigationLabel)}>
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

const styles = stylex.create({
  bottomSpacer: {
    height: "5rem",
  },
  contentLayout: {
    display: "flex",
  },
  divider: {
    borderTopColor: colors.border,
    borderTopStyle: "solid",
    borderTopWidth: "2px",
  },
  dividerContainer: {
    maxWidth: "48rem",
    paddingInline: "1.5rem",
  },
  floatingButton: {
    bottom: "1rem",
    left: "50%",
    position: "absolute",
    transform: "translateX(-50%)",
    zIndex: 20,
  },
  navigation: {
    alignSelf: "flex-start",
    display: "flex",
    flexDirection: "column",
    flexShrink: 0,
    justifyContent: "center",
    paddingBlock: "1.5rem",
    paddingInline: "0.5rem",
    position: "sticky",
    top: 0,
    width: "10rem",
  },
  navigationButton: {
    borderRadius: radii.md,
    fontSize: "0.75rem",
    paddingBlock: "0.375rem",
    paddingInline: "0.5rem",
    textAlign: "left",
    transitionDuration: "150ms",
    transitionProperty: "color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "100%",
  },
  navigationButtonActive: {
    color: colors.foreground,
    fontWeight: 500,
  },
  navigationButtonInactive: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.mutedForeground,
    },
  },
  navigationLabel: {
    WebkitBoxOrient: "vertical",
    WebkitLineClamp: 2,
    display: "-webkit-box",
    overflow: "hidden",
  },
  navigationList: {
    display: "flex",
    flexDirection: "column",
    gap: "0.125rem",
  },
  resources: {
    flex: "1",
    minWidth: 0,
  },
  scrollContainer: {
    height: "100%",
    overflow: "auto",
    position: "relative",
  },
});
