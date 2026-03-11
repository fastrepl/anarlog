import { ChevronDownIcon, ChevronUpIcon } from "lucide-react";
import { type ReactNode, useCallback, useMemo, useState } from "react";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import { Button } from "@hypr/ui/components/ui/button";
import { cn, startOfDay } from "@hypr/utils";

import { useAnchor, useAutoScrollToAnchor } from "./anchor";
import { TimelineItemComponent } from "./item";
import {
  CurrentTimeIndicator,
  useCurrentTimeMs,
  useSmartCurrentTime,
} from "./realtime";
import {
  buildTimelineBuckets,
  calculateTodayIndicatorPlacement,
  getItemTimestamp,
  type TimelineBucket,
  type TimelineIndicatorPlacement,
  type TimelineItem,
  type TimelinePrecision,
} from "./utils";

import { useConfigValue } from "~/shared/config";
import { useNativeContextMenu } from "~/shared/hooks/useNativeContextMenu";
import { useIgnoredEvents } from "~/store/tinybase/hooks";
import {
  captureSessionData,
  deleteSessionCascade,
} from "~/store/tinybase/store/deleteSession";
import * as main from "~/store/tinybase/store/main";
import { useTabs } from "~/store/zustand/tabs";
import { useTimelineSelection } from "~/store/zustand/timeline-selection";
import { useUndoDelete } from "~/store/zustand/undo-delete";

export function TimelineView() {
  const allBuckets = useTimelineData();
  const timezone = useConfigValue("timezone") || undefined;
  const [showIgnored, setShowIgnored] = useState(false);

  const { isIgnored } = useIgnoredEvents();

  const buckets = useMemo(() => {
    if (showIgnored) {
      return allBuckets;
    }

    return allBuckets
      .map((bucket) => ({
        ...bucket,
        items: bucket.items.filter((item) => {
          if (item.type !== "event") return true;
          return !isIgnored(
            item.data.tracking_id_event,
            item.data.recurrence_series_id,
          );
        }),
      }))
      .filter((bucket) => bucket.items.length > 0);
  }, [allBuckets, showIgnored, isIgnored, timezone]);

  const hasToday = useMemo(
    () => buckets.some((bucket) => bucket.label === "Today"),
    [buckets],
  );

  const currentTab = useTabs((state) => state.currentTab);

  const selectedSessionId = useMemo(() => {
    return currentTab?.type === "sessions" ? currentTab.id : undefined;
  }, [currentTab]);

  const store = main.UI.useStore(main.STORE_ID);

  const selectedIds = useTimelineSelection((s) => s.selectedIds);
  const clearSelection = useTimelineSelection((s) => s.clear);
  const indexes = main.UI.useIndexes(main.STORE_ID);
  const invalidateResource = useTabs((state) => state.invalidateResource);
  const addDeletion = useUndoDelete((state) => state.addDeletion);

  const flatItemKeys = useMemo(() => {
    const keys: string[] = [];
    for (const bucket of buckets) {
      for (const item of bucket.items) {
        keys.push(`${item.type}-${item.id}`);
      }
    }
    return keys;
  }, [buckets]);

  const {
    containerRef,
    isAnchorVisible: isTodayVisible,
    isScrolledPastAnchor: isScrolledPastToday,
    scrollToAnchor: scrollToToday,
    registerAnchor: setCurrentTimeIndicatorRef,
    anchorNode: todayAnchorNode,
  } = useAnchor();

  const todayBucketLength = useMemo(() => {
    const b = buckets.find((bucket) => bucket.label === "Today");
    return b?.items.length ?? 0;
  }, [buckets]);

  useAutoScrollToAnchor({
    scrollFn: scrollToToday,
    isVisible: isTodayVisible,
    anchorNode: todayAnchorNode,
    deps: [todayBucketLength],
  });

  const todayTimestamp = useMemo(() => startOfDay(new Date()).getTime(), []);
  const indicatorIndex = useMemo(() => {
    if (hasToday) {
      return -1;
    }
    return buckets.findIndex(
      (bucket) =>
        bucket.items.length > 0 &&
        (() => {
          const itemDate = getItemTimestamp(bucket.items[0]);
          if (!itemDate) {
            return false;
          }
          return itemDate.getTime() < todayTimestamp;
        })(),
    );
  }, [buckets, hasToday, todayTimestamp]);

  const toggleShowIgnored = useCallback(() => {
    setShowIgnored((prev) => !prev);
  }, []);

  const handleDeleteSelected = useCallback(() => {
    if (!store || !indexes) {
      return;
    }

    const sessionIds = selectedIds
      .filter((key) => key.startsWith("session-"))
      .map((key) => key.replace("session-", ""));

    const batchId = sessionIds.length > 1 ? crypto.randomUUID() : undefined;

    for (const sessionId of sessionIds) {
      const capturedData = captureSessionData(store, indexes, sessionId);

      invalidateResource("sessions", sessionId);
      void deleteSessionCascade(store, indexes, sessionId, {
        skipAudio: true,
      });

      if (capturedData) {
        addDeletion(
          capturedData,
          () => {
            void fsSyncCommands.audioDelete(sessionId);
          },
          batchId,
        );
      }
    }

    clearSelection();
  }, [
    store,
    indexes,
    selectedIds,
    invalidateResource,
    addDeletion,
    clearSelection,
  ]);

  const sessionCount = useMemo(
    () => selectedIds.filter((key) => key.startsWith("session-")).length,
    [selectedIds],
  );

  const contextMenuItems = useMemo(
    () =>
      selectedIds.length > 0
        ? [
            {
              id: "delete-selected",
              text: `Delete Selected (${sessionCount})`,
              action: handleDeleteSelected,
              disabled: sessionCount === 0,
            },
          ]
        : [
            {
              id: "toggle-ignored",
              text: showIgnored ? "Hide Ignored Events" : "Show Ignored Events",
              action: toggleShowIgnored,
            },
          ],
    [
      selectedIds,
      sessionCount,
      handleDeleteSelected,
      showIgnored,
      toggleShowIgnored,
    ],
  );

  const showContextMenu = useNativeContextMenu(contextMenuItems);

  return (
    <div className="relative h-full">
      <div
        ref={containerRef}
        onContextMenu={showContextMenu}
        className={cn([
          "scrollbar-hide flex h-full flex-col overflow-y-auto",
          "rounded-xl",
        ])}
      >
        {buckets.map((bucket, index) => {
          const isToday = bucket.label === "Today";
          const shouldRenderIndicatorBefore =
            !hasToday && indicatorIndex === index;

          return (
            <div key={bucket.label}>
              {shouldRenderIndicatorBefore && (
                <CurrentTimeIndicator
                  ref={setCurrentTimeIndicatorRef}
                  timezone={timezone}
                />
              )}
              <div
                className={cn([
                  "sticky top-0 z-10",
                  "bg-neutral-50 py-1 pr-1 pl-3",
                ])}
              >
                <div className="text-base font-bold text-neutral-900">
                  {bucket.label}
                </div>
              </div>
              {isToday ? (
                <TodayBucket
                  items={bucket.items}
                  precision={bucket.precision}
                  registerIndicator={setCurrentTimeIndicatorRef}
                  selectedSessionId={selectedSessionId}
                  timezone={timezone}
                  selectedIds={selectedIds}
                  flatItemKeys={flatItemKeys}
                />
              ) : (
                bucket.items.map((item) => {
                  const itemKey = `${item.type}-${item.id}`;
                  const selected =
                    item.type === "session" && item.id === selectedSessionId;
                  return (
                    <TimelineItemComponent
                      key={itemKey}
                      item={item}
                      precision={bucket.precision}
                      selected={selected}
                      timezone={timezone}
                      multiSelected={selectedIds.includes(itemKey)}
                      flatItemKeys={flatItemKeys}
                    />
                  );
                })
              )}
            </div>
          );
        })}
        {!hasToday &&
          (indicatorIndex === -1 || indicatorIndex === buckets.length) && (
            <CurrentTimeIndicator
              ref={setCurrentTimeIndicatorRef}
              timezone={timezone}
            />
          )}
      </div>

      {!isTodayVisible && (
        <Button
          onClick={scrollToToday}
          size="sm"
          className={cn([
            "absolute left-1/2 -translate-x-1/2 transform",
            "rounded-full bg-white hover:bg-neutral-50",
            "border border-neutral-200 text-neutral-700",
            "z-20 flex items-center gap-1",
            "shadow-xs",
            isScrolledPastToday ? "top-2" : "bottom-2",
          ])}
          variant="outline"
        >
          {!isScrolledPastToday ? (
            <ChevronDownIcon size={12} />
          ) : (
            <ChevronUpIcon size={12} />
          )}
          <span className="text-xs">Go back to now</span>
        </Button>
      )}
    </div>
  );
}

function TodayBucket({
  items,
  precision,
  registerIndicator,
  selectedSessionId,
  timezone,
  selectedIds,
  flatItemKeys,
}: {
  items: TimelineItem[];
  precision: TimelinePrecision;
  registerIndicator: (node: HTMLDivElement | null) => void;
  selectedSessionId: string | undefined;
  timezone?: string;
  selectedIds: string[];
  flatItemKeys: string[];
}) {
  const currentTimeMs = useCurrentTimeMs();

  const entries = useMemo(
    () =>
      items.map((timelineItem) => ({
        item: timelineItem,
        timestamp: getItemTimestamp(timelineItem),
      })),
    [items],
  );

  const indicatorPlacement = useMemo<TimelineIndicatorPlacement>(
    // currentTimeMs in deps triggers updates as time passes,
    // but we use fresh Date() so indicator positions correctly when entries change immediately (new note).
    () => calculateTodayIndicatorPlacement(entries, new Date()),
    [entries, currentTimeMs],
  );

  const renderedEntries = useMemo(() => {
    if (entries.length === 0) {
      return (
        <>
          <CurrentTimeIndicator ref={registerIndicator} timezone={timezone} />
          <div className="px-3 py-4 text-center text-sm text-neutral-400">
            No items today
          </div>
        </>
      );
    }

    const nodes: ReactNode[] = [];

    entries.forEach((entry, index) => {
      if (
        indicatorPlacement.type === "before" &&
        index === indicatorPlacement.index
      ) {
        nodes.push(
          <CurrentTimeIndicator
            ref={registerIndicator}
            key="current-time-indicator"
            timezone={timezone}
          />,
        );
      }

      const itemKey = `${entry.item.type}-${entry.item.id}`;
      const selected =
        entry.item.type === "session" && entry.item.id === selectedSessionId;

      const itemNode = (
        <TimelineItemComponent
          key={itemKey}
          item={entry.item}
          precision={precision}
          selected={selected}
          timezone={timezone}
          multiSelected={selectedIds.includes(itemKey)}
          flatItemKeys={flatItemKeys}
        />
      );

      if (
        indicatorPlacement.type === "inside" &&
        index === indicatorPlacement.index
      ) {
        nodes.push(
          <div key={`${itemKey}-wrapper`} className="relative">
            <CurrentTimeIndicator
              ref={registerIndicator}
              key="current-time-indicator-inside"
              timezone={timezone}
              variant="inside"
              progress={indicatorPlacement.progress}
            />
            {itemNode}
          </div>,
        );
        return;
      }

      nodes.push(itemNode);
    });

    if (indicatorPlacement.type === "after") {
      nodes.push(
        <CurrentTimeIndicator
          ref={registerIndicator}
          key="current-time-indicator-end"
          timezone={timezone}
        />,
      );
    }

    return <>{nodes}</>;
  }, [
    entries,
    indicatorPlacement,
    precision,
    registerIndicator,
    selectedSessionId,
    timezone,
    selectedIds,
    flatItemKeys,
  ]);

  return renderedEntries;
}

function useTimelineData(): TimelineBucket[] {
  const timelineEventsTable = main.UI.useResultTable(
    main.QUERIES.timelineEvents,
    main.STORE_ID,
  );
  const timelineSessionsTable = main.UI.useResultTable(
    main.QUERIES.timelineSessions,
    main.STORE_ID,
  );
  const currentTimeMs = useSmartCurrentTime(
    timelineEventsTable,
    timelineSessionsTable,
  );
  const timezone = useConfigValue("timezone");

  return useMemo(
    () =>
      buildTimelineBuckets({
        timelineEventsTable,
        timelineSessionsTable,
        timezone: timezone || undefined,
      }),
    [timelineEventsTable, timelineSessionsTable, currentTimeMs, timezone],
  );
}
