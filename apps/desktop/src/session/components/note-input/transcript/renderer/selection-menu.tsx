import {
  autoUpdate,
  flip,
  FloatingPortal,
  offset,
  shift,
  useFloating,
} from "@floating-ui/react";
import { t } from "@lingui/core/macro";
import { Trans } from "@lingui/react/macro";
import { ArrowsMerge, Play, UserSwitch, X } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import {
  type MouseEvent,
  useCallback,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { createPortal } from "react-dom";

import { colors, radii, shadows } from "@anlg/design-system/tokens.stylex";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@anlg/ui/components/ui/popover";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

import { getTranscriptSelectionFromRange } from "./selection";
import type { TranscriptWordSelection } from "./selection";
import { SpeakerParticipantPicker } from "./speaker-assign";

import {
  getSessionFabSelectionHost,
  subscribeSessionFabSelectionHost,
} from "~/session/components/floating/selection-slot";
import { useAutoCloser } from "~/shared/hooks/useAutoCloser";
import { useMountEffect } from "~/shared/hooks/useMountEffect";

export type TranscriptContextMenuRequest = {
  id: string;
  range: Range;
  selection: TranscriptWordSelection;
  x: number;
  y: number;
};

export function SelectionMenu({
  containerRef,
  contextRequest,
  audioExists,
  onContextClose,
  onAction,
  onAssignSpeaker,
}: {
  containerRef: React.RefObject<HTMLElement | null>;
  contextRequest: TranscriptContextMenuRequest | null;
  audioExists: boolean;
  onContextClose: () => void;
  onAction?: (
    action: "copy" | "play",
    selection: TranscriptWordSelection,
  ) => void;
  onAssignSpeaker?: (
    selection: TranscriptWordSelection,
    humanId: string,
  ) => void | Promise<void>;
}) {
  return (
    <>
      <TextSelectionMenu
        containerRef={containerRef}
        suspended={contextRequest !== null}
        audioExists={audioExists}
        onAction={onAction}
        onAssignSpeaker={onAssignSpeaker}
      />
      {contextRequest && (
        <ContextSelectionMenu
          key={contextRequest.id}
          request={contextRequest}
          containerRef={containerRef}
          audioExists={audioExists}
          onClose={onContextClose}
          onAction={onAction}
          onAssignSpeaker={onAssignSpeaker}
        />
      )}
    </>
  );
}

export function MultiSelectionBar({
  selection,
  entryCount,
  canMerge = false,
  onClear,
  onAssignSpeaker,
  onMerge,
}: {
  selection: TranscriptWordSelection;
  entryCount: number;
  canMerge?: boolean;
  onClear: () => void;
  onAssignSpeaker: (
    selection: TranscriptWordSelection,
    humanId: string,
  ) => void | Promise<void>;
  onMerge?: () => void | Promise<void>;
}) {
  const fabSelectionHost = useSyncExternalStore(
    subscribeSessionFabSelectionHost,
    getSessionFabSelectionHost,
    getSessionFabSelectionHost,
  );
  const [speakerPickerOpen, setSpeakerPickerOpen] = useState(false);
  const handleAssign = useCallback(
    async (humanId: string) => {
      await onAssignSpeaker(selection, humanId);
      setSpeakerPickerOpen(false);
      onClear();
    },
    [onAssignSpeaker, onClear, selection],
  );
  const handleMerge = useCallback(async () => {
    await onMerge?.();
    onClear();
  }, [onClear, onMerge]);

  const bar = (
    <div
      {...stylex.props(
        styles.selectionBar,
        !fabSelectionHost && styles.floating,
      )}
    >
      <span {...stylex.props(styles.selectionCount)}>
        <Trans>{entryCount} selected</Trans>
      </span>
      <Popover open={speakerPickerOpen} onOpenChange={setSpeakerPickerOpen}>
        <PopoverTrigger asChild>
          <button type="button" {...stylex.props(styles.primaryButton)}>
            <UserSwitch {...stylex.props(styles.icon)} />
            <Trans>Change speaker</Trans>
          </button>
        </PopoverTrigger>
        <PopoverContent
          variant="app"
          side="top"
          align="center"
          sideOffset={8}
          sx={styles.speakerPopover}
        >
          <SpeakerParticipantPicker
            sessionId={selection.sessionId}
            showAssignmentScope={false}
            onSelect={handleAssign}
          />
        </PopoverContent>
      </Popover>
      {onMerge ? (
        <button
          type="button"
          disabled={!canMerge}
          {...stylex.props(styles.secondaryButton)}
          onClick={() => void handleMerge()}
        >
          <ArrowsMerge {...stylex.props(styles.icon)} />
          <Trans>Merge</Trans>
        </button>
      ) : null}
      <button
        type="button"
        aria-label={t`Clear selection`}
        {...stylex.props(styles.clearButton)}
        onClick={onClear}
      >
        <X {...stylex.props(styles.icon)} />
      </button>
    </div>
  );

  return fabSelectionHost ? createPortal(bar, fabSelectionHost) : bar;
}

function TextSelectionMenu({
  containerRef,
  suspended,
  audioExists,
  onAction,
  onAssignSpeaker,
}: {
  containerRef: React.RefObject<HTMLElement | null>;
  suspended: boolean;
  audioExists: boolean;
  onAction?: (
    action: "copy" | "play",
    selection: TranscriptWordSelection,
  ) => void;
  onAssignSpeaker?: (
    selection: TranscriptWordSelection,
    humanId: string,
  ) => void | Promise<void>;
}) {
  const { isVisible, selection, hide, refs, floatingStyles, storedRange } =
    useSelectionMenuState({ containerRef });
  const handleClose = useCallback(() => {
    hide();
    window.getSelection()?.removeAllRanges();
  }, [hide]);
  const autoCloserRef = useAutoCloser(handleClose, {
    esc: true,
    outside: true,
  });
  const floatingRef = useCallback(
    (node: HTMLDivElement | null) => {
      refs.setFloating(node);
      autoCloserRef.current = node;
    },
    [refs, autoCloserRef],
  );

  if (!isVisible || !selection || suspended) {
    return null;
  }

  return (
    <SelectionFloatingMenu
      selection={selection}
      range={storedRange}
      containerRef={containerRef}
      floatingRef={floatingRef}
      floatingStyles={floatingStyles}
      audioExists={audioExists}
      onClose={handleClose}
      onAction={onAction}
      onAssignSpeaker={onAssignSpeaker}
    />
  );
}

function ContextSelectionMenu({
  request,
  containerRef,
  audioExists,
  onClose,
  onAction,
  onAssignSpeaker,
}: {
  request: TranscriptContextMenuRequest;
  containerRef: React.RefObject<HTMLElement | null>;
  audioExists: boolean;
  onClose: () => void;
  onAction?: (
    action: "copy" | "play",
    selection: TranscriptWordSelection,
  ) => void;
  onAssignSpeaker?: (
    selection: TranscriptWordSelection,
    humanId: string,
  ) => void | Promise<void>;
}) {
  const virtualRect = useMemo(
    () => new DOMRect(request.x, request.y, 0, 0),
    [request.x, request.y],
  );
  const { refs, floatingStyles, update } = useFloating<HTMLElement>({
    open: true,
    placement: "bottom-start",
    strategy: "fixed",
    transform: false,
    middleware: [offset(4), flip(), shift({ padding: 8 })],
    whileElementsMounted: autoUpdate,
  });
  const handleClose = useCallback(() => {
    onClose();
    window.getSelection()?.removeAllRanges();
  }, [onClose]);
  const autoCloserRef = useAutoCloser(handleClose, {
    esc: true,
    outside: true,
  });
  const floatingRef = useCallback(
    (node: HTMLDivElement | null) => {
      refs.setFloating(node);
      refs.setPositionReference({
        getBoundingClientRect: () => virtualRect,
      });
      autoCloserRef.current = node;
      if (node) {
        void update();
      }
    },
    [autoCloserRef, refs, update, virtualRect],
  );

  return (
    <SelectionFloatingMenu
      selection={request.selection}
      range={request.range}
      containerRef={containerRef}
      floatingRef={floatingRef}
      floatingStyles={floatingStyles}
      audioExists={audioExists}
      onClose={handleClose}
      onAction={onAction}
      onAssignSpeaker={onAssignSpeaker}
    />
  );
}

function SelectionFloatingMenu({
  selection,
  range,
  containerRef,
  floatingRef,
  floatingStyles,
  audioExists,
  onClose,
  onAction,
  onAssignSpeaker,
}: {
  selection: TranscriptWordSelection;
  range: Range | null;
  containerRef: React.RefObject<HTMLElement | null>;
  floatingRef: (node: HTMLDivElement | null) => void;
  floatingStyles: React.CSSProperties;
  audioExists: boolean;
  onClose: () => void;
  onAction?: (
    action: "copy" | "play",
    selection: TranscriptWordSelection,
  ) => void;
  onAssignSpeaker?: (
    selection: TranscriptWordSelection,
    humanId: string,
  ) => void | Promise<void>;
}) {
  const [view, setView] = useState<"actions" | "speaker">("actions");
  const handleAction = useCallback(
    (action: "copy" | "play") => {
      onAction?.(action, selection);
      onClose();
    },
    [onAction, onClose, selection],
  );
  const handleAssign = useCallback(
    async (humanId: string) => {
      await onAssignSpeaker?.(selection, humanId);
      onClose();
    },
    [onAssignSpeaker, onClose, selection],
  );
  const handleMouseDown = useCallback((event: MouseEvent<HTMLDivElement>) => {
    event.preventDefault();
  }, []);

  return (
    <>
      <SelectionHighlight
        key={getSelectionHighlightKey(selection, range)}
        range={range}
        containerRef={containerRef}
      />
      <FloatingPortal>
        <div
          ref={floatingRef}
          {...mergeStyleXProps(
            [
              styles.menu,
              view === "speaker" ? styles.speakerMenu : styles.actionsMenu,
            ],
            undefined,
            { ...floatingStyles, zIndex: 50 },
          )}
          onMouseDown={view === "actions" ? handleMouseDown : undefined}
        >
          {view === "actions" ? (
            <div {...stylex.props(styles.actionList)}>
              {selection.sessionId && onAssignSpeaker && (
                <button
                  type="button"
                  {...stylex.props(styles.menuButton)}
                  onClick={() => setView("speaker")}
                >
                  <UserSwitch {...stylex.props(styles.icon)} />
                  <Trans>Change speaker</Trans>
                </button>
              )}
              {audioExists && (
                <button
                  type="button"
                  {...stylex.props(styles.menuButton)}
                  onClick={() => handleAction("play")}
                >
                  <Play {...stylex.props(styles.icon)} />
                  <Trans>Play from here</Trans>
                </button>
              )}
              <button
                type="button"
                {...stylex.props(styles.menuButton)}
                onClick={() => handleAction("copy")}
              >
                <span {...stylex.props(styles.commandIcon)}>⌘</span>
                <Trans>Copy</Trans>
              </button>
            </div>
          ) : (
            <SpeakerParticipantPicker
              sessionId={selection.sessionId}
              showAssignmentScope={false}
              onSelect={handleAssign}
            />
          )}
        </div>
      </FloatingPortal>
    </>
  );
}

function SelectionHighlight({
  range,
  containerRef,
}: {
  range: Range | null;
  containerRef: React.RefObject<HTMLElement | null>;
}) {
  const [rects, setRects] = useState<DOMRect[]>([]);

  const updateRects = useCallback(() => {
    if (!range) {
      setRects([]);
      return;
    }

    setRects(Array.from(range.getClientRects()));
  }, [range]);

  useMountEffect(() => {
    if (!range) {
      return;
    }

    updateRects();
    const container = containerRef.current;
    window.addEventListener("resize", updateRects);
    container?.addEventListener("scroll", updateRects, { passive: true });

    return () => {
      window.removeEventListener("resize", updateRects);
      container?.removeEventListener("scroll", updateRects);
    };
  });

  if (rects.length === 0) {
    return null;
  }

  return (
    <FloatingPortal>
      {rects.map((rect, index) => (
        <div
          key={index}
          style={{
            position: "fixed",
            left: rect.left,
            top: rect.top,
            width: rect.width,
            height: rect.height,
            backgroundColor: "var(--selection-overlay)",
            pointerEvents: "none",
            zIndex: 40,
          }}
        />
      ))}
    </FloatingPortal>
  );
}

function useSelectionMenuState({
  containerRef,
}: {
  containerRef: React.RefObject<HTMLElement | null>;
}) {
  const [selection, setSelection] = useState<TranscriptWordSelection | null>(
    null,
  );
  const [storedRange, setStoredRange] = useState<Range | null>(null);
  const isVisible = selection !== null;
  const { refs, floatingStyles, update } = useFloating<HTMLElement>({
    open: isVisible,
    placement: "bottom",
    strategy: "fixed",
    transform: false,
    middleware: [offset(6), flip(), shift({ padding: 8 })],
    whileElementsMounted: autoUpdate,
  });
  const show = useCallback(
    (nextSelection: TranscriptWordSelection, range: Range) => {
      setSelection(nextSelection);
      setStoredRange(range.cloneRange());
      refs.setPositionReference({
        getBoundingClientRect: () => range.getBoundingClientRect(),
      });
    },
    [refs],
  );
  const hide = useCallback(() => {
    setSelection(null);
    setStoredRange(null);
  }, []);
  const handleSelectionChangeRef = useRef<() => void>(() => {});
  handleSelectionChangeRef.current = () => {
    const nativeSelection = window.getSelection();
    const container = containerRef.current;
    if (!nativeSelection || nativeSelection.rangeCount === 0 || !container) {
      if (!isVisibleRef.current) {
        hide();
      }
      return;
    }

    const range = nativeSelection.getRangeAt(0);
    const nextSelection = getTranscriptSelectionFromRange(range, container);
    if (!nextSelection?.text) {
      if (!isVisibleRef.current) {
        hide();
      }
      return;
    }

    show(nextSelection, range);
  };
  const updateRef = useRef(update);
  updateRef.current = update;
  const isVisibleRef = useRef(isVisible);
  isVisibleRef.current = isVisible;

  useMountEffect(() => {
    const handleSelectionChange = () => handleSelectionChangeRef.current();
    document.addEventListener("selectionchange", handleSelectionChange);
    return () => {
      document.removeEventListener("selectionchange", handleSelectionChange);
    };
  });

  useMountEffect(() => {
    const container = containerRef.current;
    if (!container) {
      return;
    }
    const handleScroll = () => {
      if (isVisibleRef.current) {
        void updateRef.current();
      }
    };
    container.addEventListener("scroll", handleScroll, { passive: true });
    return () => {
      container.removeEventListener("scroll", handleScroll);
    };
  });

  return { isVisible, selection, hide, refs, floatingStyles, storedRange };
}

function getSelectionHighlightKey(
  selection: TranscriptWordSelection,
  range: Range | null,
) {
  return [
    ...selection.groups.flatMap((group) => group.wordIds),
    selection.text,
    range?.startOffset ?? 0,
    range?.endOffset ?? 0,
  ].join(":");
}

const styles = stylex.create({
  actionList: {
    display: "flex",
    flexDirection: "column",
    gap: "0.125rem",
  },
  actionsMenu: {
    minWidth: "10rem",
  },
  clearButton: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: radii.full,
    display: "flex",
    height: "1.75rem",
    justifyContent: "center",
    width: "1.75rem",
  },
  commandIcon: {
    textAlign: "center",
    width: "0.875rem",
  },
  floating: {
    bottom: "1rem",
    left: "50%",
    position: "absolute",
    transform: "translateX(-50%)",
    zIndex: 40,
  },
  icon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  menu: {
    backgroundColor: colors.card,
    borderColor: colors.border,
    borderRadius: radii.md,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: shadows.lg,
    padding: "0.25rem",
    pointerEvents: "auto",
  },
  menuButton: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: "0.125rem",
    display: "flex",
    fontSize: "0.75rem",
    gap: "0.5rem",
    paddingBlock: "0.375rem",
    paddingInline: "0.5rem",
    textAlign: "left",
    transitionDuration: "150ms",
    transitionProperty: "background-color",
    width: "100%",
  },
  primaryButton: {
    alignItems: "center",
    backgroundColor: {
      default: colors.primary,
      ":hover": `color-mix(in srgb, ${colors.primary} 90%, transparent)`,
    },
    borderRadius: radii.full,
    color: colors.primaryForeground,
    display: "flex",
    fontWeight: 500,
    gap: "0.375rem",
    height: "1.75rem",
    paddingInline: "0.75rem",
  },
  secondaryButton: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: radii.full,
    display: "flex",
    fontWeight: 500,
    gap: "0.375rem",
    height: "1.75rem",
    opacity: {
      default: 1,
      ":disabled": 0.5,
    },
    paddingInline: "0.5rem",
    pointerEvents: {
      default: "auto",
      ":disabled": "none",
    },
  },
  selectionBar: {
    alignItems: "center",
    backgroundColor: colors.card,
    borderColor: colors.border,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: shadows.lg,
    display: "flex",
    fontSize: "0.75rem",
    gap: "0.5rem",
    paddingBottom: "0.25rem",
    paddingLeft: "0.75rem",
    paddingRight: "0.25rem",
    paddingTop: "0.25rem",
  },
  selectionCount: {
    color: colors.mutedForeground,
    whiteSpace: "nowrap",
  },
  speakerMenu: {
    maxHeight: "min(28rem, calc(100vh - 1rem))",
    width: "20rem",
  },
  speakerPopover: {
    width: "20rem",
  },
});

export { styles as selectionMenuStyles };
