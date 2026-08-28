import * as stylex from "@stylexjs/stylex";
import { type ReactNode, type RefCallback, useMemo } from "react";

import { colors, spacing } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

import { TimelineItemComponent } from "./item";
import { CurrentTimeIndicator, useCurrentTimeMs } from "./realtime";
import type { SidebarUpcomingMeetingStatus } from "./upcoming-meeting";
import {
  calculateTodayIndicatorPlacement,
  getItemTimestamp,
  type TimelineBucket,
  type TimelineIndicatorPlacement,
  type TimelineItem,
  type TimelinePrecision,
} from "./utils";

export function TimelineBuckets({
  bucketHeaderTopSx,
  buckets,
  emptyTodayLabel,
  getFlatItemKeys,
  hasActiveVisibleSession,
  hasToday,
  indicatorIndex,
  registerIndicator,
  selectedIds,
  selectedNodeRef,
  selectedSessionId,
  timezone,
  upcomingMeetingStatus,
  upcomingNodeRef,
}: {
  bucketHeaderTopSx: StyleXProps["sx"];
  buckets: TimelineBucket[];
  emptyTodayLabel: ReactNode;
  getFlatItemKeys: () => string[];
  hasActiveVisibleSession: boolean;
  hasToday: boolean;
  indicatorIndex: number;
  registerIndicator: (node: HTMLDivElement | null) => void;
  selectedIds: string[];
  selectedNodeRef: RefCallback<HTMLDivElement>;
  selectedSessionId: string | undefined;
  timezone?: string;
  upcomingMeetingStatus: SidebarUpcomingMeetingStatus | null;
  upcomingNodeRef: RefCallback<HTMLDivElement>;
}) {
  return (
    <>
      {buckets.map((bucket, index) => {
        const isToday = bucket.label === "Today";
        const shouldPlaceIndicatorBefore =
          !hasToday && indicatorIndex === index;
        const shouldRenderIndicatorBefore =
          shouldPlaceIndicatorBefore && !hasActiveVisibleSession;
        const shouldRenderIndicatorAnchorBefore =
          shouldPlaceIndicatorBefore && hasActiveVisibleSession;
        const isTopIndicator = shouldRenderIndicatorBefore && index === 0;

        return (
          <div
            key={bucket.label}
            {...stylex.props(isTopIndicator && styles.topIndicatorBucket)}
          >
            {shouldRenderIndicatorBefore && (
              <div
                data-sidebar-current-time-header-gap
                {...stylex.props(styles.indicatorHeaderGap)}
              >
                <CurrentTimeIndicator
                  ref={registerIndicator}
                  timezone={timezone}
                />
              </div>
            )}
            {shouldRenderIndicatorAnchorBefore && (
              <CurrentTimeAnchor registerIndicator={registerIndicator} />
            )}
            <div
              data-sidebar-timeline-bucket-header
              {...stylex.props(styles.bucketHeader, bucketHeaderTopSx)}
            >
              <div {...stylex.props(styles.bucketLabel)}>{bucket.label}</div>
            </div>
            {isToday ? (
              <TodayBucket
                items={bucket.items}
                precision={bucket.precision}
                emptyLabel={emptyTodayLabel}
                registerIndicator={registerIndicator}
                selectedSessionId={selectedSessionId}
                selectedNodeRef={selectedNodeRef}
                suppressCurrentTimeIndicator={hasActiveVisibleSession}
                timezone={timezone}
                selectedIds={selectedIds}
                getFlatItemKeys={getFlatItemKeys}
                upcomingItemKey={upcomingMeetingStatus?.itemKey}
                upcomingItemLabel={upcomingMeetingStatus?.label}
                upcomingItemProgress={upcomingMeetingStatus?.progress}
                upcomingItemNodeRef={upcomingNodeRef}
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
                    getFlatItemKeys={getFlatItemKeys}
                    selectedNodeRef={selected ? selectedNodeRef : undefined}
                    itemNodeRef={
                      itemKey === upcomingMeetingStatus?.itemKey
                        ? upcomingNodeRef
                        : undefined
                    }
                    isUpcoming={itemKey === upcomingMeetingStatus?.itemKey}
                    upcomingLabel={
                      itemKey === upcomingMeetingStatus?.itemKey
                        ? upcomingMeetingStatus.label
                        : undefined
                    }
                    upcomingProgress={
                      itemKey === upcomingMeetingStatus?.itemKey
                        ? upcomingMeetingStatus.progress
                        : undefined
                    }
                  />
                );
              })
            )}
          </div>
        );
      })}
      {!hasToday &&
        (indicatorIndex === -1 || indicatorIndex === buckets.length) &&
        (hasActiveVisibleSession ? (
          <CurrentTimeAnchor registerIndicator={registerIndicator} />
        ) : (
          <CurrentTimeIndicator ref={registerIndicator} timezone={timezone} />
        ))}
    </>
  );
}

function CurrentTimeAnchor({
  progress = 0.5,
  registerIndicator,
  variant = "seam",
}: {
  progress?: number;
  registerIndicator: (node: HTMLDivElement | null) => void;
  variant?: "seam" | "inside";
}) {
  return (
    <div
      ref={registerIndicator}
      aria-hidden
      data-sidebar-current-time-anchor
      {...mergeStyleXProps(
        [
          styles.anchor,
          variant === "inside" ? styles.anchorInside : styles.anchorSeam,
        ],
        undefined,
        variant === "inside" ? { top: `${(1 - progress) * 100}%` } : undefined,
      )}
    />
  );
}

function TodayBucket({
  emptyLabel,
  items,
  precision,
  registerIndicator,
  selectedSessionId,
  selectedNodeRef,
  suppressCurrentTimeIndicator,
  timezone,
  selectedIds,
  getFlatItemKeys,
  upcomingItemKey,
  upcomingItemLabel,
  upcomingItemProgress,
  upcomingItemNodeRef,
}: {
  emptyLabel: ReactNode;
  items: TimelineItem[];
  precision: TimelinePrecision;
  registerIndicator: (node: HTMLDivElement | null) => void;
  selectedSessionId: string | undefined;
  selectedNodeRef: RefCallback<HTMLDivElement>;
  suppressCurrentTimeIndicator: boolean;
  timezone?: string;
  selectedIds: string[];
  getFlatItemKeys: () => string[];
  upcomingItemKey?: string;
  upcomingItemLabel?: string;
  upcomingItemProgress?: number;
  upcomingItemNodeRef: RefCallback<HTMLDivElement>;
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
          {suppressCurrentTimeIndicator ? (
            <CurrentTimeAnchor registerIndicator={registerIndicator} />
          ) : (
            <CurrentTimeIndicator ref={registerIndicator} timezone={timezone} />
          )}
          <div {...stylex.props(styles.empty)}>{emptyLabel}</div>
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
          suppressCurrentTimeIndicator ? (
            <CurrentTimeAnchor
              key="current-time-anchor"
              registerIndicator={registerIndicator}
            />
          ) : (
            <CurrentTimeIndicator
              ref={registerIndicator}
              key="current-time-indicator"
              timezone={timezone}
            />
          ),
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
          getFlatItemKeys={getFlatItemKeys}
          selectedNodeRef={selected ? selectedNodeRef : undefined}
          itemNodeRef={
            itemKey === upcomingItemKey ? upcomingItemNodeRef : undefined
          }
          isUpcoming={itemKey === upcomingItemKey}
          upcomingLabel={
            itemKey === upcomingItemKey ? upcomingItemLabel : undefined
          }
          upcomingProgress={
            itemKey === upcomingItemKey ? upcomingItemProgress : undefined
          }
        />
      );

      if (
        indicatorPlacement.type === "inside" &&
        index === indicatorPlacement.index
      ) {
        nodes.push(
          <div key={`${itemKey}-wrapper`} {...stylex.props(styles.itemWrapper)}>
            {suppressCurrentTimeIndicator ? (
              <CurrentTimeAnchor
                registerIndicator={registerIndicator}
                variant="inside"
                progress={indicatorPlacement.progress}
              />
            ) : (
              <CurrentTimeIndicator
                ref={registerIndicator}
                key="current-time-indicator-inside"
                timezone={timezone}
                variant="inside"
                progress={indicatorPlacement.progress}
              />
            )}
            {itemNode}
          </div>,
        );
        return;
      }

      nodes.push(itemNode);
    });

    if (indicatorPlacement.type === "after") {
      nodes.push(
        suppressCurrentTimeIndicator ? (
          <CurrentTimeAnchor
            key="current-time-anchor-end"
            registerIndicator={registerIndicator}
          />
        ) : (
          <CurrentTimeIndicator
            ref={registerIndicator}
            key="current-time-indicator-end"
            timezone={timezone}
          />
        ),
      );
    }

    return <>{nodes}</>;
  }, [
    entries,
    emptyLabel,
    indicatorPlacement,
    precision,
    registerIndicator,
    selectedSessionId,
    selectedNodeRef,
    suppressCurrentTimeIndicator,
    timezone,
    selectedIds,
    getFlatItemKeys,
    upcomingItemKey,
    upcomingItemLabel,
    upcomingItemProgress,
    upcomingItemNodeRef,
  ]);

  return renderedEntries;
}

const styles = stylex.create({
  anchor: {
    height: "1px",
    opacity: 0,
    pointerEvents: "none",
    zIndex: 20,
  },
  anchorInside: {
    left: 0,
    position: "absolute",
    right: 0,
  },
  anchorSeam: {
    position: "relative",
  },
  bucketHeader: {
    backgroundColor: colors.background,
    paddingBottom: spacing.xs,
    paddingLeft: spacing.md,
    paddingRight: spacing.xs,
    paddingTop: 0,
    position: "sticky",
    zIndex: 20,
  },
  bucketLabel: {
    color: colors.foreground,
    fontSize: "1rem",
    fontWeight: 700,
  },
  empty: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    paddingBlock: "1rem",
    paddingInline: spacing.md,
    textAlign: "center",
  },
  indicatorHeaderGap: {
    paddingBlock: spacing.md,
  },
  itemWrapper: {
    position: "relative",
  },
  topIndicatorBucket: {
    paddingTop: spacing.md,
  },
});

export { styles as timelineBucketStyles };
