import { useLingui } from "@lingui/react/macro";
import { Lock, LockOpen, Square, Users } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { platform } from "@tauri-apps/plugin-os";
import {
  createContext,
  memo,
  type DragEvent,
  type RefCallback,
  useCallback,
  useContext,
  useMemo,
  useState,
} from "react";

import {
  colors,
  fonts,
  radii,
  spacing,
} from "@anlg/design-system/tokens.stylex";
import { commands as fsSyncCommands } from "@anlg/plugin-fs-sync";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { DancingSticks } from "@anlg/ui/components/ui/dancing-sticks";
import { Spinner } from "@anlg/ui/components/ui/spinner";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";
import { format, getYear, safeParseDate, TZDate } from "@anlg/utils";

import {
  type EventTimelineItem,
  isTimelineItemInFuture,
  type SessionTimelineItem,
  type TimelineItem,
  TimelinePrecision,
} from "./utils";

import { useIgnoredEvents } from "~/calendar/ignored-events";
import { writeSessionContextDragData } from "~/chat/context/session-drag";
import { DEVICE_AUTH_REASON } from "~/lock/auth";
import { isLockedFlag } from "~/lock/flag";
import { revealLockedNote, setSessionLocked } from "~/lock/notes";
import { useAppLock } from "~/lock/store";
import { useDeleteSession } from "~/session/hooks/useDeleteSession";
import { useIsSessionEnhancing } from "~/session/hooks/useEnhancedNotes";
import {
  getOrCreateSessionForEventId,
  preloadSession,
} from "~/session/queries";
import { getSessionEvent } from "~/session/utils";
import { openStandaloneNoteWindow } from "~/session/window";
import type { MenuItemDef } from "~/shared/hooks/useNativeContextMenu";
import { InteractiveButton } from "~/shared/ui/interactive-button";
import { useSessionTitle } from "~/store/zustand/live-title";
import { useTabs } from "~/store/zustand/tabs";
import { useTimelineSelection } from "~/store/zustand/timeline-selection";
import { useListener } from "~/stt/contexts";

const EMPTY_TIMELINE_ITEM_KEYS: string[] = [];
const EMPTY_MANAGED_SHARED_SESSION_IDS = new Set<string>();

export const ManagedSharedSessionIdsContext = createContext<
  ReadonlySet<string>
>(EMPTY_MANAGED_SHARED_SESSION_IDS);

type ItemBaseProps = {
  title: string;
  displayTime: string;
  isLive?: boolean;
  amplitude?: number;
  showSpinner?: boolean;
  isShared?: boolean;
  isLocked?: boolean;
  isLockRevealed?: boolean;
  selected: boolean;
  ignored?: boolean;
  muted?: boolean;
  multiSelected: boolean;
  onClick: () => void;
  onDoubleClick?: () => void;
  onCmdClick: () => void;
  onShiftClick: () => void;
  onStop?: () => void;
  onDragStart?: (event: DragEvent<HTMLElement>) => void;
  contextMenu: MenuItemDef[];
  draggable?: boolean;
  selectedNodeRef?: RefCallback<HTMLDivElement>;
  itemNodeRef?: RefCallback<HTMLDivElement>;
  timelineSessionId?: string;
  isUpcoming?: boolean;
  upcomingProgress?: number;
  onPreload?: () => void;
};

export const TimelineItemComponent = memo(
  ({
    item,
    precision,
    selected,
    timezone,
    multiSelected,
    flatItemKeys,
    getFlatItemKeys,
    selectedNodeRef,
    itemNodeRef,
    isUpcoming,
    upcomingProgress,
  }: {
    item: TimelineItem;
    precision: TimelinePrecision;
    selected: boolean;
    timezone?: string;
    multiSelected: boolean;
    flatItemKeys?: string[];
    getFlatItemKeys?: () => string[];
    selectedNodeRef?: RefCallback<HTMLDivElement>;
    itemNodeRef?: RefCallback<HTMLDivElement>;
    isUpcoming?: boolean;
    upcomingLabel?: string;
    upcomingProgress?: number;
  }) => {
    const readFlatItemKeys =
      getFlatItemKeys ?? (() => flatItemKeys ?? EMPTY_TIMELINE_ITEM_KEYS);

    if (item.type === "event") {
      return (
        <EventItem
          item={item}
          precision={precision}
          selected={selected}
          timezone={timezone}
          multiSelected={multiSelected}
          getFlatItemKeys={readFlatItemKeys}
          selectedNodeRef={selectedNodeRef}
          itemNodeRef={itemNodeRef}
          isUpcoming={isUpcoming}
          upcomingProgress={upcomingProgress}
        />
      );
    }
    return (
      <SessionItem
        item={item}
        precision={precision}
        selected={selected}
        timezone={timezone}
        multiSelected={multiSelected}
        getFlatItemKeys={readFlatItemKeys}
        selectedNodeRef={selectedNodeRef}
        itemNodeRef={itemNodeRef}
        isUpcoming={isUpcoming}
        upcomingProgress={upcomingProgress}
      />
    );
  },
);

const ItemBase = memo(function ItemBase({
  title,
  displayTime,
  isLive,
  amplitude,
  showSpinner,
  isShared,
  isLocked,
  isLockRevealed,
  selected,
  ignored,
  muted,
  multiSelected,
  onClick,
  onDoubleClick,
  onCmdClick,
  onShiftClick,
  onStop,
  onDragStart,
  contextMenu,
  draggable,
  selectedNodeRef,
  itemNodeRef,
  timelineSessionId,
  isUpcoming,
  upcomingProgress,
  onPreload,
}: ItemBaseProps) {
  const { t } = useLingui();
  const hasSelection = useTimelineSelection((s) => s.selectedIds.length > 0);
  const showLiveStop = isLive && onStop;
  const showUpcomingGauge =
    typeof upcomingProgress === "number" &&
    Boolean(isUpcoming) &&
    !isLive &&
    !showSpinner;
  const upcomingGaugePercent =
    typeof upcomingProgress === "number"
      ? Math.round(Math.max(0, Math.min(upcomingProgress, 1)) * 100)
      : 0;
  const showTrailingStatus = showLiveStop || showSpinner;
  const setItemRef = useCallback(
    (node: HTMLDivElement | null) => {
      selectedNodeRef?.(node);
      itemNodeRef?.(node);
    },
    [selectedNodeRef, itemNodeRef],
  );

  return (
    <div
      ref={setItemRef}
      data-sidebar-timeline-session-id={timelineSessionId}
      onFocus={onPreload}
      onPointerDown={onPreload}
      {...stylex.props(styles.root, stylex.defaultMarker())}
    >
      <InteractiveButton
        onClick={ignored ? undefined : onClick}
        onDoubleClick={ignored ? undefined : onDoubleClick}
        onCmdClick={ignored ? undefined : onCmdClick}
        onShiftClick={ignored ? undefined : onShiftClick}
        onDragStart={onDragStart}
        contextMenu={hasSelection ? undefined : contextMenu}
        sx={[
          styles.item,
          showUpcomingGauge && styles.itemWithGauge,
          showTrailingStatus && styles.itemWithTrailingStatus,
          ignored ? styles.itemIgnoredCursor : styles.itemCursor,
          (multiSelected || (!multiSelected && selected)) &&
            styles.itemSelected,
          !multiSelected && !selected && styles.itemIdle,
          isUpcoming && !isLive && styles.itemUpcoming,
          isLive && styles.itemLive,
          ignored && styles.itemIgnored,
          !ignored && muted && !isLive && !isUpcoming && styles.itemMuted,
        ]}
        draggable={draggable}
      >
        <div {...stylex.props(styles.itemRow)}>
          <div {...stylex.props(styles.itemText)}>
            <div
              {...stylex.props(
                styles.itemTitle,
                ignored && styles.itemTitleIgnored,
              )}
            >
              {title || t`Untitled`}
            </div>
            {displayTime && (
              <div
                {...stylex.props(
                  styles.itemTime,
                  isLive ? styles.itemTimeLive : styles.itemTimeDefault,
                )}
              >
                {displayTime}
              </div>
            )}
          </div>
          {isLocked ? (
            isLockRevealed ? (
              <LockOpen
                aria-label={t`Unlock Note`}
                {...stylex.props(styles.statusIcon)}
                weight="fill"
              />
            ) : (
              <Lock
                aria-label={t`Locked note`}
                {...stylex.props(styles.statusIcon)}
                weight="fill"
              />
            )
          ) : null}
          {isShared ? (
            <Users
              aria-label={t`Shared note`}
              {...stylex.props(styles.statusIcon)}
            />
          ) : null}
        </div>
      </InteractiveButton>
      {showUpcomingGauge ? (
        <div
          aria-hidden
          data-sidebar-timeline-upcoming-gauge
          {...stylex.props(styles.upcomingGauge)}
        >
          <div
            data-sidebar-timeline-upcoming-gauge-fill
            {...mergeStyleXProps(styles.upcomingGaugeFill, undefined, {
              height: `${upcomingGaugePercent}%`,
            })}
          />
        </div>
      ) : null}
      {showSpinner ? (
        <div aria-hidden {...stylex.props(styles.trailingSlot)}>
          <Spinner size={14} />
        </div>
      ) : null}
      {showLiveStop ? (
        <button
          type="button"
          aria-label={t`Stop listening`}
          onClick={(event) => {
            event.preventDefault();
            event.stopPropagation();
            onStop();
          }}
          {...stylex.props(styles.stopButton)}
        >
          <span aria-hidden {...stylex.props(styles.liveIndicator)}>
            <DancingSticks
              amplitude={amplitude ?? 0.25}
              color="currentColor"
              height={14}
              width={13}
              stickWidth={2}
              gap={2}
            />
          </span>
          <span aria-hidden {...stylex.props(styles.stopIndicator)}>
            <Square size={10} weight="fill" />
          </span>
        </button>
      ) : null}
    </div>
  );
}, itemBasePropsAreEqual);

function itemBasePropsAreEqual(prev: ItemBaseProps, next: ItemBaseProps) {
  return (
    prev.title === next.title &&
    prev.displayTime === next.displayTime &&
    prev.isLive === next.isLive &&
    prev.amplitude === next.amplitude &&
    prev.showSpinner === next.showSpinner &&
    prev.isShared === next.isShared &&
    prev.isLocked === next.isLocked &&
    prev.isLockRevealed === next.isLockRevealed &&
    prev.selected === next.selected &&
    prev.ignored === next.ignored &&
    prev.muted === next.muted &&
    prev.multiSelected === next.multiSelected &&
    prev.onClick === next.onClick &&
    prev.onDoubleClick === next.onDoubleClick &&
    prev.onCmdClick === next.onCmdClick &&
    prev.onShiftClick === next.onShiftClick &&
    prev.onStop === next.onStop &&
    prev.onDragStart === next.onDragStart &&
    prev.contextMenu === next.contextMenu &&
    prev.draggable === next.draggable &&
    prev.selectedNodeRef === next.selectedNodeRef &&
    prev.itemNodeRef === next.itemNodeRef &&
    prev.timelineSessionId === next.timelineSessionId &&
    prev.isUpcoming === next.isUpcoming &&
    prev.upcomingProgress === next.upcomingProgress &&
    prev.onPreload === next.onPreload
  );
}

const EventItem = memo(
  ({
    item,
    precision,
    selected,
    timezone,
    multiSelected,
    getFlatItemKeys,
    selectedNodeRef,
    itemNodeRef,
    isUpcoming,
    upcomingProgress,
  }: {
    item: EventTimelineItem;
    precision: TimelinePrecision;
    selected: boolean;
    timezone?: string;
    multiSelected: boolean;
    getFlatItemKeys: () => string[];
    selectedNodeRef?: RefCallback<HTMLDivElement>;
    itemNodeRef?: RefCallback<HTMLDivElement>;
    isUpcoming?: boolean;
    upcomingProgress?: number;
  }) => {
    const { t } = useLingui();
    const openCurrent = useTabs((state) => state.openCurrent);

    const eventId = item.id;
    const trackingIdEvent = item.data.tracking_id_event;
    const title = item.data.title || t`Untitled`;
    const recurrenceSeriesId = item.data.recurrence_series_id;

    const {
      isIgnored,
      ignoreEvent,
      unignoreEvent,
      ignoreSeries,
      unignoreSeries,
    } = useIgnoredEvents();

    const ignored = isIgnored(trackingIdEvent, recurrenceSeriesId);

    const displayTime = useMemo(
      () => formatDisplayTime(item.data.started_at, precision, timezone),
      [item.data.started_at, precision, timezone],
    );

    const [isOpening, setIsOpening] = useState(false);
    const openEvent = useCallback(() => {
      if (!eventId || isOpening) return;
      setIsOpening(true);
      void getOrCreateSessionForEventId(eventId, title)
        .then((sessionId) => {
          openCurrent({ id: sessionId, type: "sessions" });
        })
        .catch((error) => {
          console.error("[timeline] failed to open event note", error);
        })
        .finally(() => {
          setIsOpening(false);
        });
    }, [eventId, title, openCurrent, isOpening]);

    const itemKey = `event-${item.id}`;
    const muted = isTimelineItemInFuture(item);

    const handleClick = useCallback(() => {
      useTimelineSelection.getState().setAnchor(itemKey);
      openEvent();
    }, [openEvent, itemKey]);

    const handleCmdClick = useCallback(() => {
      useTimelineSelection.getState().toggleSelect(itemKey);
    }, [itemKey]);

    const handleShiftClick = useCallback(() => {
      useTimelineSelection.getState().selectRange(getFlatItemKeys(), itemKey);
    }, [getFlatItemKeys, itemKey]);

    const handleIgnore = useCallback(() => {
      if (!trackingIdEvent) return;
      ignoreEvent(trackingIdEvent);
    }, [trackingIdEvent, ignoreEvent]);

    const handleUnignore = useCallback(() => {
      if (!trackingIdEvent) return;
      unignoreEvent(trackingIdEvent);
    }, [trackingIdEvent, unignoreEvent]);

    const handleUnignoreSeries = useCallback(() => {
      if (!recurrenceSeriesId) return;
      unignoreSeries(recurrenceSeriesId);
    }, [recurrenceSeriesId, unignoreSeries]);

    const handleIgnoreSeries = useCallback(() => {
      if (!recurrenceSeriesId) return;
      ignoreSeries(recurrenceSeriesId);
    }, [recurrenceSeriesId, ignoreSeries]);

    const contextMenu = useMemo(() => {
      if (ignored) {
        if (recurrenceSeriesId) {
          return [
            {
              id: "unignore",
              text: t`Show This Event`,
              action: handleUnignore,
            },
            {
              id: "unignore-series",
              text: t`Show All Recurring Events`,
              action: handleUnignoreSeries,
            },
          ];
        }
        return [
          { id: "unignore", text: t`Show Event`, action: handleUnignore },
        ];
      }
      const menu: MenuItemDef[] = [
        {
          id: "ignore",
          text: recurrenceSeriesId ? t`Delete This Event` : t`Delete Event`,
          action: handleIgnore,
        },
      ];
      if (recurrenceSeriesId) {
        menu.push({
          id: "ignore-series",
          text: t`Delete All Recurring Events`,
          action: handleIgnoreSeries,
        });
      }
      return menu;
    }, [
      ignored,
      handleIgnore,
      handleUnignore,
      handleUnignoreSeries,
      handleIgnoreSeries,
      recurrenceSeriesId,
      t,
    ]);

    return (
      <ItemBase
        title={title}
        displayTime={displayTime}
        showSpinner={isOpening}
        selected={selected}
        ignored={ignored}
        muted={muted}
        multiSelected={multiSelected}
        onClick={handleClick}
        onCmdClick={handleCmdClick}
        onShiftClick={handleShiftClick}
        contextMenu={contextMenu}
        selectedNodeRef={selected ? selectedNodeRef : undefined}
        itemNodeRef={itemNodeRef}
        isUpcoming={isUpcoming}
        upcomingProgress={upcomingProgress}
      />
    );
  },
);

const SessionItem = memo(
  ({
    item,
    precision,
    selected,
    timezone,
    multiSelected,
    getFlatItemKeys,
    selectedNodeRef,
    itemNodeRef,
    isUpcoming,
    upcomingProgress,
  }: {
    item: SessionTimelineItem;
    precision: TimelinePrecision;
    selected: boolean;
    timezone?: string;
    multiSelected: boolean;
    getFlatItemKeys: () => string[];
    selectedNodeRef?: RefCallback<HTMLDivElement>;
    itemNodeRef?: RefCallback<HTMLDivElement>;
    isUpcoming?: boolean;
    upcomingProgress?: number;
  }) => {
    const { t } = useLingui();
    const openCurrent = useTabs((state) => state.openCurrent);
    const deleteSession = useDeleteSession();
    const managedSharedSessionIds = useContext(ManagedSharedSessionIdsContext);

    const sessionId = item.id;
    const title = useSessionTitle(sessionId, item.data.title ?? undefined);
    const noteLocked = isLockedFlag(item.data.locked);
    const noteRevealed = useAppLock((state) =>
      Boolean(state.revealedNoteIds[sessionId]),
    );
    const authAvailable = useAppLock((state) => state.available) === true;

    const { sessionMode, stop, amplitude } = useListener((state) => {
      const sessionMode = state.getSessionMode(sessionId);
      return {
        sessionMode,
        stop: state.stop,
        amplitude: sessionMode === "active" ? state.live.amplitude : null,
      };
    });
    const isEnhancing = useIsSessionEnhancing(sessionId);
    const isLive = sessionMode === "active";
    const isFinalizing = sessionMode === "finalizing";
    const isBatching = sessionMode === "running_batch";
    const [isOpening, setIsOpening] = useState(false);
    const showSpinner =
      !selected &&
      !isLive &&
      (isFinalizing || isEnhancing || isBatching || isOpening);

    const sessionEvent = getSessionEvent(item.data);

    const displayTime = useMemo(
      () =>
        formatDisplayTime(
          sessionEvent?.started_at ?? item.data.created_at,
          precision,
          timezone,
        ),
      [sessionEvent?.started_at, item.data.created_at, precision, timezone],
    );
    const muted = isTimelineItemInFuture(item);

    const itemKey = `session-${item.id}`;

    const handlePreload = useCallback(() => {
      if (!noteLocked) void preloadSession(sessionId).catch(() => {});
    }, [noteLocked, sessionId]);

    const openSession = useCallback(async () => {
      setIsOpening(true);
      try {
        await preloadSession(sessionId);
      } catch (error) {
        console.error("[timeline] failed to preload session", error);
      } finally {
        openCurrent({ id: sessionId, type: "sessions" });
        setIsOpening(false);
      }
    }, [openCurrent, sessionId]);

    const handleClick = useCallback(() => {
      useTimelineSelection.getState().setAnchor(itemKey);
      if (noteLocked) {
        void revealLockedNote(sessionId).then((ok) => {
          if (ok) void openSession();
        });
        return;
      }
      void openSession();
    }, [noteLocked, sessionId, openSession, itemKey]);

    const handleCmdClick = useCallback(() => {
      useTimelineSelection.getState().toggleSelect(itemKey);
    }, [itemKey]);

    const handleShiftClick = useCallback(() => {
      useTimelineSelection.getState().selectRange(getFlatItemKeys(), itemKey);
    }, [getFlatItemKeys, itemKey]);

    const handleOpenStandaloneWindow = useCallback(() => {
      void openStandaloneNoteWindow(sessionId);
    }, [sessionId]);

    const handleDragStart = useCallback(
      (event: DragEvent<HTMLElement>) => {
        writeSessionContextDragData(
          event.dataTransfer,
          sessionId,
          title || t`Untitled`,
        );
      },
      [sessionId, title, t],
    );

    const handleDelete = useCallback(() => {
      deleteSession(sessionId, {
        trackingId: sessionEvent?.tracking_id,
        title,
      });
    }, [deleteSession, sessionId, sessionEvent?.tracking_id, title]);

    const handleToggleLock = useCallback(() => {
      void setSessionLocked(sessionId, !noteLocked);
    }, [noteLocked, sessionId]);

    const handleShowInFolder = useCallback(async () => {
      if (noteLocked) {
        const ok = await useAppLock
          .getState()
          .authenticate(DEVICE_AUTH_REASON.openApp);
        if (!ok) return;
      }
      const result = await fsSyncCommands.sessionDir(sessionId);
      if (result.status === "ok") {
        await openerCommands.openPath(result.data, null);
      }
    }, [noteLocked, sessionId]);

    const contextMenu = useMemo(() => {
      const menu: MenuItemDef[] = [
        {
          id: "open-new-window",
          text: t`Open in New Window`,
          action: handleOpenStandaloneWindow,
        },
        {
          id: "show",
          text: platform() === "macos" ? t`Show in Finder` : t`Show in folder`,
          action: handleShowInFolder,
        },
      ];
      if (authAvailable) {
        menu.push({
          id: noteLocked ? "unlock" : "lock",
          text: noteLocked ? t`Unlock Note` : t`Lock Note`,
          action: handleToggleLock,
        });
      }
      menu.push(
        { separator: true as const },
        {
          id: "delete",
          text: t`Delete Note`,
          action: handleDelete,
        },
      );
      return menu;
    }, [
      authAvailable,
      handleDelete,
      handleOpenStandaloneWindow,
      handleShowInFolder,
      handleToggleLock,
      noteLocked,
      t,
    ]);

    return (
      <ItemBase
        title={title}
        displayTime={displayTime}
        isLive={isLive}
        amplitude={Math.max(
          0.25,
          Math.min(Math.hypot(amplitude?.mic ?? 0, amplitude?.speaker ?? 0), 1),
        )}
        showSpinner={showSpinner}
        isShared={managedSharedSessionIds.has(sessionId)}
        isLocked={noteLocked}
        isLockRevealed={noteRevealed}
        selected={selected}
        muted={muted}
        multiSelected={multiSelected}
        onClick={handleClick}
        onDoubleClick={handleOpenStandaloneWindow}
        onCmdClick={handleCmdClick}
        onShiftClick={handleShiftClick}
        onStop={stop}
        onDragStart={handleDragStart}
        contextMenu={contextMenu}
        selectedNodeRef={selected ? selectedNodeRef : undefined}
        itemNodeRef={itemNodeRef}
        timelineSessionId={sessionId}
        isUpcoming={isUpcoming}
        upcomingProgress={upcomingProgress}
        onPreload={handlePreload}
        draggable
      />
    );
  },
);

function formatDisplayTime(
  timestamp: string | null | undefined,
  precision: TimelinePrecision,
  timezone?: string,
): string {
  const parsed = safeParseDate(timestamp);
  if (!parsed) {
    return "";
  }

  const date = timezone ? new TZDate(parsed, timezone) : parsed;
  const time = format(date, "h:mm a").toUpperCase();

  if (precision === "time") {
    return time;
  }

  const now = timezone ? new TZDate(new Date(), timezone) : new Date();
  const sameYear = getYear(date) === getYear(now);
  const dateStr = sameYear
    ? format(date, "MMM d")
    : format(date, "MMM d, yyyy");

  return `${dateStr}, ${time}`;
}

const styles = stylex.create({
  item: {
    borderRadius: radii.lg,
    paddingBlock: spacing.sm,
    paddingInline: spacing.md,
    textAlign: "left",
    width: "100%",
  },
  itemCursor: {
    cursor: "pointer",
  },
  itemIdle: {
    backgroundColor: {
      default: null,
      ":hover": `color-mix(in oklab, ${colors.accent} 50%, transparent)`,
    },
  },
  itemIgnored: {
    opacity: 0.4,
  },
  itemIgnoredCursor: {
    cursor: "default",
  },
  itemLive: {
    backgroundColor: {
      default: colors.destructive,
      ":hover": `color-mix(in oklab, ${colors.destructive} 90%, transparent)`,
    },
    boxShadow: {
      default: null,
      ":focus-visible": `0 0 0 2px color-mix(in oklab, ${colors.destructive} 40%, transparent)`,
    },
    color: colors.destructiveForeground,
    outline: {
      default: null,
      ":focus-visible": "2px solid transparent",
    },
    outlineOffset: {
      default: null,
      ":focus-visible": "2px",
    },
  },
  itemMuted: {
    opacity: 0.65,
  },
  itemRow: {
    alignItems: "center",
    display: "flex",
    gap: spacing.sm,
  },
  itemSelected: {
    backgroundColor: colors.accent,
  },
  itemText: {
    display: "flex",
    flex: "1",
    flexDirection: "column",
    gap: "0.125rem",
    minWidth: 0,
  },
  itemTime: {
    fontFamily: fonts.mono,
    fontSize: "0.75rem",
  },
  itemTimeDefault: {
    color: colors.mutedForeground,
  },
  itemTimeLive: {
    color: `color-mix(in oklab, ${colors.destructiveForeground} 65%, transparent)`,
  },
  itemTitle: {
    fontSize: "0.875rem",
    fontWeight: 400,
    minWidth: 0,
    overflow: "hidden",
    pointerEvents: "none",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  itemTitleIgnored: {
    textDecorationLine: "line-through",
  },
  itemUpcoming: {
    backgroundColor: `color-mix(in oklab, ${colors.destructive} 8%, transparent)`,
    boxShadow: {
      default: null,
      ":focus-visible": `0 0 0 1px color-mix(in oklab, ${colors.destructive} 25%, transparent)`,
    },
    color: colors.foreground,
  },
  itemWithGauge: {
    paddingLeft: "1rem",
  },
  itemWithTrailingStatus: {
    paddingRight: "2.5rem",
  },
  liveIndicator: {
    alignItems: "center",
    display: {
      default: "flex",
      [stylex.when.ancestor(":hover")]: "none",
    },
    justifyContent: "center",
  },
  root: {
    containIntrinsicSize: "auto 56px",
    contentVisibility: "auto",
    position: "relative",
  },
  statusIcon: {
    color: colors.mutedForeground,
    flexShrink: 0,
    height: "0.875rem",
    width: "0.875rem",
  },
  stopButton: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": "rgb(255 255 255 / 0.15)",
    },
    borderRadius: radii.sm,
    boxShadow: {
      default: null,
      ":focus-visible": "0 0 0 2px rgb(255 255 255 / 0.7)",
    },
    color: {
      default: "rgb(255 255 255 / 0.8)",
      ":hover": "white",
    },
    display: "flex",
    height: "1.25rem",
    justifyContent: "center",
    outline: {
      default: null,
      ":focus-visible": "2px solid transparent",
    },
    outlineOffset: {
      default: null,
      ":focus-visible": "2px",
    },
    position: "absolute",
    right: spacing.md,
    top: "50%",
    transform: "translateY(-50%)",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "1.25rem",
  },
  stopIndicator: {
    alignItems: "center",
    display: {
      default: "none",
      [stylex.when.ancestor(":hover")]: "flex",
    },
    justifyContent: "center",
  },
  trailingSlot: {
    alignItems: "center",
    color: colors.mutedForeground,
    display: "flex",
    height: "1.25rem",
    justifyContent: "center",
    pointerEvents: "none",
    position: "absolute",
    right: spacing.md,
    top: "50%",
    transform: "translateY(-50%)",
    width: "1.25rem",
  },
  upcomingGauge: {
    backgroundColor: `color-mix(in oklab, ${colors.destructive} 20%, transparent)`,
    borderRadius: radii.full,
    bottom: spacing.sm,
    left: "0.375rem",
    overflow: "hidden",
    pointerEvents: "none",
    position: "absolute",
    top: spacing.sm,
    width: "0.125rem",
  },
  upcomingGaugeFill: {
    backgroundColor: colors.destructive,
    borderRadius: radii.full,
    bottom: 0,
    left: 0,
    position: "absolute",
    transitionDuration: "300ms",
    transitionProperty: "height",
    transitionTimingFunction: "linear",
    width: "100%",
  },
});

export { styles as timelineItemStyles };
