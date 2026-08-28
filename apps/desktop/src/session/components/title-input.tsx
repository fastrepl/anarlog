import { Trans, useLingui } from "@lingui/react/macro";
import * as stylex from "@stylexjs/stylex";
import { usePrevious } from "@uidotdev/usehooks";
import {
  type CSSProperties,
  forwardRef,
  memo,
  useCallback,
  useEffect,
  useImperativeHandle,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { useResizeObserver } from "usehooks-ts";

import { colors } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

import { useTitleGenerating } from "~/ai/hooks";
import { useSession, useUpdateSession } from "~/session/queries";
import { useLiveTitle } from "~/store/zustand/live-title";
import { type Tab } from "~/store/zustand/tabs";

export interface TitleInputHandle {
  focus: () => void;
  focusAtEnd: () => void;
  focusAtPixelWidth: (pixelWidth: number) => void;
}

export const TitleInput = forwardRef<
  TitleInputHandle,
  {
    tab: Extract<Tab, { type: "sessions" }>;
    onTransferContentToEditor?: (content: string) => void;
    onFocusEditorAtStart?: () => void;
    onFocusEditorAtPixelWidth?: (pixelWidth: number) => void;
    variant?: "title" | "breadcrumb";
  }
>(
  (
    {
      tab,
      onTransferContentToEditor,
      onFocusEditorAtStart,
      onFocusEditorAtPixelWidth,
      variant = "title",
    },
    ref,
  ) => {
    const {
      id: sessionId,
      state: { view },
    } = tab;
    const isGenerating = useTitleGenerating(sessionId);
    const wasGenerating = usePrevious(isGenerating);
    const [showRevealAnimation, setShowRevealAnimation] = useState(false);
    const [generatedTitle, setGeneratedTitle] = useState<string | null>(null);
    const storeTitle = useSession(sessionId)?.title;

    const editorId = view ? "active" : "inactive";
    const inputRef = useRef<TitleInputHandle>(null);

    useImperativeHandle(ref, () => inputRef.current!, []);

    useEffect(() => {
      if (wasGenerating && !isGenerating) {
        setGeneratedTitle(storeTitle ?? null);
        setShowRevealAnimation(true);
        const timer = setTimeout(() => {
          setShowRevealAnimation(false);
        }, 1000);
        return () => clearTimeout(timer);
      }
    }, [wasGenerating, isGenerating, storeTitle]);

    if (isGenerating) {
      return (
        <div
          data-tauri-drag-region="false"
          {...stylex.props(
            styles.state,
            variant === "breadcrumb"
              ? styles.breadcrumbHeight
              : styles.titleHeight,
          )}
        >
          <span
            {...stylex.props(
              styles.generating,
              variant === "breadcrumb"
                ? styles.breadcrumbText
                : styles.titleText,
            )}
          >
            <Trans>Generating title...</Trans>
          </span>
        </div>
      );
    }

    if (showRevealAnimation && generatedTitle) {
      return (
        <div
          data-tauri-drag-region="false"
          {...stylex.props(
            styles.state,
            styles.stateClipped,
            variant === "breadcrumb"
              ? styles.breadcrumbHeight
              : styles.titleHeight,
          )}
        >
          <span
            {...stylex.props(
              styles.reveal,
              variant === "breadcrumb"
                ? styles.breadcrumbText
                : styles.titleText,
            )}
          >
            {generatedTitle}
          </span>
        </div>
      );
    }

    return (
      <TitleInputInner
        key={sessionId}
        ref={inputRef}
        sessionId={sessionId}
        editorId={editorId}
        onTransferContentToEditor={onTransferContentToEditor}
        onFocusEditorAtStart={onFocusEditorAtStart}
        onFocusEditorAtPixelWidth={onFocusEditorAtPixelWidth}
        variant={variant}
      />
    );
  },
);

const TitleInputInner = memo(
  forwardRef<
    TitleInputHandle,
    {
      sessionId: string;
      editorId: string;
      placeholder?: string;
      onTransferContentToEditor?: (content: string) => void;
      onFocusEditorAtStart?: () => void;
      onFocusEditorAtPixelWidth?: (pixelWidth: number) => void;
      variant: "title" | "breadcrumb";
    }
  >(
    (
      {
        sessionId,
        editorId,
        placeholder,
        onTransferContentToEditor,
        onFocusEditorAtStart,
        onFocusEditorAtPixelWidth,
        variant,
      },
      ref,
    ) => {
      const { t } = useLingui();
      const untitled = placeholder ?? t`Untitled`;
      const storeTitle = useSession(sessionId)?.title;
      const updateSession = useUpdateSession(sessionId);
      const [draftTitle, setDraftTitle] = useState<string | null>(null);
      const [isOverflowing, setIsOverflowing] = useState(false);
      const [overflowDistance, setOverflowDistance] = useState(0);
      const [showStartFade, setShowStartFade] = useState(false);
      const [showEndFade, setShowEndFade] = useState(false);
      const [isTitleFocused, setIsTitleFocused] = useState(false);
      const internalRef = useRef<HTMLInputElement>(null);
      const editRevisionRef = useRef(0);
      const setLiveTitle = useLiveTitle((s) => s.setTitle);
      const markLiveTitlePersisted = useLiveTitle((s) => s.markTitlePersisted);
      const clearLiveTitle = useLiveTitle((s) => s.clearTitle);
      const title = draftTitle ?? storeTitle ?? "";

      const updateOverflowState = useCallback(
        (node?: HTMLInputElement | null) => {
          const input = node ?? internalRef.current;
          if (!input) {
            setIsOverflowing(false);
            setOverflowDistance(0);
            setShowStartFade(false);
            setShowEndFade(false);
            return;
          }
          const distance = Math.max(input.scrollWidth - input.clientWidth, 0);
          const overflowing = distance > 1;
          const scrollLeft = Math.max(input.scrollLeft, 0);
          setIsOverflowing(distance > 1);
          setOverflowDistance(distance);
          setShowStartFade(overflowing && scrollLeft > 1);
          setShowEndFade(overflowing && scrollLeft < distance - 1);
        },
        [],
      );

      const setInputRef = useCallback(
        (node: HTMLInputElement | null) => {
          internalRef.current = node;
          if (node) {
            requestAnimationFrame(() => updateOverflowState(node));
          } else {
            setIsOverflowing(false);
            setOverflowDistance(0);
            setShowStartFade(false);
            setShowEndFade(false);
          }
        },
        [updateOverflowState],
      );

      useResizeObserver({
        ref: internalRef as React.RefObject<HTMLInputElement>,
        onResize: () => updateOverflowState(),
      });

      const titleFadeStyle =
        showStartFade || showEndFade
          ? {
              WebkitMaskImage: getTitleFadeMask({
                showStartFade,
                showEndFade,
              }),
              maskImage: getTitleFadeMask({ showStartFade, showEndFade }),
              WebkitMaskRepeat: "no-repeat",
              maskRepeat: "no-repeat",
              WebkitMaskSize: "100% 100%",
              maskSize: "100% 100%",
            }
          : undefined;
      const showHoverReveal =
        isOverflowing && !isTitleFocused && title.length > 0;
      const titleHoverScrollStyle = showHoverReveal
        ? ({
            "--title-hover-scroll-distance": `-${Math.ceil(overflowDistance)}px`,
            "--title-hover-scroll-duration": `${Math.min(
              Math.max(overflowDistance / 48, 2.5),
              8,
            ).toFixed(2)}s`,
          } as CSSProperties)
        : undefined;
      const visibleTitleLength = Math.max(
        title.length || untitled.length,
        untitled.length,
      );
      const titleShellStyle = {
        ...titleFadeStyle,
        width: `calc(${visibleTitleLength}ch + 2px)`,
      };

      useImperativeHandle(
        ref,
        () => ({
          focus: () => internalRef.current?.focus(),
          focusAtEnd: () => {
            const input = internalRef.current;
            if (input) {
              input.focus();
              input.setSelectionRange(input.value.length, input.value.length);
            }
          },
          focusAtPixelWidth: (pixelWidth: number) => {
            const input = internalRef.current;
            if (input && input.value) {
              input.focus();
              const titleStyle = window.getComputedStyle(input);
              const canvas = document.createElement("canvas");
              const ctx = canvas.getContext("2d");
              if (ctx) {
                ctx.font = `${titleStyle.fontWeight} ${titleStyle.fontSize} ${titleStyle.fontFamily}`;
                let charPos = 0;
                for (let i = 0; i <= input.value.length; i++) {
                  const currentWidth = ctx.measureText(
                    input.value.slice(0, i),
                  ).width;
                  if (currentWidth >= pixelWidth) {
                    charPos = i;
                    break;
                  }
                  charPos = i;
                }
                input.setSelectionRange(charPos, charPos);
              }
            } else if (input) {
              input.focus();
            }
          },
        }),
        [],
      );

      useLayoutEffect(() => {
        requestAnimationFrame(() => updateOverflowState());
      }, [title, updateOverflowState]);

      const setStoreTitle = useCallback(
        (title: string) => updateSession({ title }),
        [updateSession],
      );

      const persistTitle = useCallback(
        (value: string) => {
          const editRevision = editRevisionRef.current;
          const previousTitle = storeTitle;
          return setStoreTitle(value)
            .then(() => {
              if (editRevisionRef.current !== editRevision) return;
              markLiveTitlePersisted(sessionId, value, previousTitle);
            })
            .catch((error) => {
              console.error("[title-input] failed to persist title", error);
              if (editRevisionRef.current !== editRevision) return;
              clearLiveTitle(sessionId);
            })
            .finally(() => {
              if (editRevisionRef.current !== editRevision) return;
              setDraftTitle(null);
            });
        },
        [
          clearLiveTitle,
          markLiveTitlePersisted,
          sessionId,
          setStoreTitle,
          storeTitle,
        ],
      );

      const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
        if (isComposingKeyEvent(e)) {
          return;
        }

        if (e.key === "ArrowUp") {
          e.preventDefault();
          return;
        }

        if (e.key === "Enter") {
          e.preventDefault();
          if (!onTransferContentToEditor && !onFocusEditorAtStart) {
            editRevisionRef.current += 1;
            e.currentTarget.blur();
            return;
          }

          const input = internalRef.current;
          if (!input) return;

          const cursorPos = input.selectionStart ?? input.value.length;
          const beforeCursor = input.value.slice(0, cursorPos);
          const afterCursor = input.value.slice(cursorPos);

          editRevisionRef.current += 1;
          setDraftTitle(beforeCursor);
          setLiveTitle(sessionId, beforeCursor);
          void persistTitle(beforeCursor);

          if (afterCursor) {
            setTimeout(() => onTransferContentToEditor?.(afterCursor), 0);
          } else {
            setTimeout(() => onFocusEditorAtStart?.(), 0);
          }
        } else if (e.key === "Tab" && onFocusEditorAtStart) {
          e.preventDefault();
          setTimeout(() => onFocusEditorAtStart?.(), 0);
        } else if (e.key === "ArrowRight" && onFocusEditorAtStart) {
          const input = internalRef.current;
          if (!input) return;
          const cursorPos = input.selectionStart ?? 0;
          if (
            cursorPos === input.value.length &&
            input.selectionEnd === cursorPos
          ) {
            e.preventDefault();
            setTimeout(() => onFocusEditorAtStart?.(), 0);
          }
        } else if (e.key === "ArrowDown" && onFocusEditorAtPixelWidth) {
          e.preventDefault();
          const input = internalRef.current;
          if (!input) return;

          const cursorPos = input.selectionStart ?? 0;
          const textBeforeCursor = input.value.slice(0, cursorPos);

          const canvas = document.createElement("canvas");
          const ctx = canvas.getContext("2d");
          if (ctx) {
            const titleStyle = window.getComputedStyle(input);
            ctx.font = `${titleStyle.fontWeight} ${titleStyle.fontSize} ${titleStyle.fontFamily}`;
            const titleWidth = ctx.measureText(textBeforeCursor).width;
            setTimeout(() => onFocusEditorAtPixelWidth?.(titleWidth), 0);
          }
        }
      };

      return (
        <div
          data-tauri-drag-region="false"
          {...mergeStyleXProps(
            [
              styles.shell,
              stylex.defaultMarker(),
              variant === "breadcrumb"
                ? [styles.breadcrumbHeight, styles.breadcrumbText]
                : [styles.titleHeight, styles.titleText],
            ],
            undefined,
            titleShellStyle,
          )}
        >
          <input
            data-tauri-drag-region="false"
            data-session-title-input
            aria-label={t`Session title`}
            ref={setInputRef}
            id={`title-input-${sessionId}-${editorId}`}
            placeholder={untitled}
            type="text"
            onChange={(e) => {
              const value = e.target.value;
              editRevisionRef.current += 1;
              setDraftTitle(value);
              setLiveTitle(sessionId, value);
              updateOverflowState(e.target);
            }}
            onClick={(e) => updateOverflowState(e.currentTarget)}
            onKeyDown={handleKeyDown}
            onKeyUp={(e) => updateOverflowState(e.currentTarget)}
            onFocus={() => {
              setIsTitleFocused(true);
              updateOverflowState();
            }}
            onBlur={(e) => {
              setIsTitleFocused(false);
              if (draftTitle !== null) {
                void persistTitle(e.target.value);
              }
              updateOverflowState(e.target);
            }}
            onScroll={(e) => updateOverflowState(e.currentTarget)}
            onSelect={(e) => updateOverflowState(e.currentTarget)}
            value={title}
            size={visibleTitleLength}
            {...stylex.props(
              styles.input,
              variant === "breadcrumb"
                ? [
                    styles.breadcrumbHeight,
                    styles.breadcrumbText,
                    styles.breadcrumbInput,
                    isTitleFocused
                      ? styles.breadcrumbInputFocused
                      : styles.breadcrumbInputTruncated,
                  ]
                : styles.titleText,
              showHoverReveal && styles.concealedInput,
            )}
          />
          {showHoverReveal ? (
            <div aria-hidden="true" {...stylex.props(styles.hoverOverlay)}>
              <span
                {...mergeStyleXProps(
                  [
                    styles.hoverTitle,
                    variant === "breadcrumb"
                      ? styles.breadcrumbText
                      : styles.titleText,
                  ],
                  undefined,
                  titleHoverScrollStyle,
                )}
              >
                {title}
              </span>
            </div>
          ) : null}
        </div>
      );
    },
  ),
);

function isComposingKeyEvent(event: React.KeyboardEvent<HTMLInputElement>) {
  return (
    event.nativeEvent.isComposing ||
    event.key === "Process" ||
    event.keyCode === 229
  );
}

function getTitleFadeMask({
  showStartFade,
  showEndFade,
}: {
  showStartFade: boolean;
  showEndFade: boolean;
}) {
  if (showStartFade && showEndFade) {
    return "linear-gradient(to right, transparent 0, black 28px, black calc(100% - 28px), transparent 100%)";
  }

  if (showStartFade) {
    return "linear-gradient(to right, transparent 0, black 28px, black 100%)";
  }

  return "linear-gradient(to right, black 0, black calc(100% - 28px), transparent 100%)";
}

const pulse = stylex.keyframes({
  "0%, 100%": {
    opacity: 1,
  },
  "50%": {
    opacity: 0.5,
  },
});

const revealLeft = stylex.keyframes({
  "0%": {
    clipPath: "inset(0 100% 0 0)",
  },
  "100%": {
    clipPath: "inset(0 0 0 0)",
  },
});

const titleHoverScroll = stylex.keyframes({
  "0%, 16%": {
    transform: "translateX(0)",
  },
  "84%, 100%": {
    transform: "translateX(var(--title-hover-scroll-distance, 0px))",
  },
});

const styles = stylex.create({
  breadcrumbHeight: {
    height: "1.25rem",
  },
  breadcrumbInput: {
    appearance: "none",
    color: colors.foreground,
    padding: 0,
    textDecorationLine: {
      default: null,
      ":focus": "underline",
    },
  },
  breadcrumbInputFocused: {
    overflowX: "auto",
    whiteSpace: "nowrap",
  },
  breadcrumbInputTruncated: {
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  breadcrumbText: {
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
  },
  concealedInput: {
    caretColor: "transparent",
    color: "transparent",
  },
  generating: {
    animationDuration: "2s",
    animationIterationCount: "infinite",
    animationName: pulse,
    animationTimingFunction: "cubic-bezier(0.4, 0, 0.6, 1)",
    color: colors.mutedForeground,
  },
  hoverOverlay: {
    alignItems: "center",
    display: "flex",
    inset: 0,
    justifyContent: "flex-start",
    overflow: "hidden",
    pointerEvents: "none",
    position: "absolute",
  },
  hoverTitle: {
    animationDuration: {
      default: null,
      [stylex.when.ancestor(":hover")]:
        "var(--title-hover-scroll-duration, 4s)",
    },
    animationFillMode: {
      default: null,
      [stylex.when.ancestor(":hover")]: "forwards",
    },
    animationName: {
      default: null,
      [stylex.when.ancestor(":hover")]: titleHoverScroll,
    },
    animationTimingFunction: {
      default: null,
      [stylex.when.ancestor(":hover")]: "ease-in-out",
    },
    whiteSpace: "nowrap",
    willChange: {
      default: null,
      [stylex.when.ancestor(":hover")]: "transform",
    },
  },
  input: {
    "::placeholder": {
      color: colors.mutedForeground,
    },
    backgroundColor: "transparent",
    borderStyle: "none",
    minWidth: 0,
    outlineStyle: {
      default: null,
      ":focus": "none",
    },
    textAlign: "left",
    transitionDuration: "200ms",
    transitionProperty: "opacity",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "100%",
  },
  reveal: {
    animationDuration: "500ms",
    animationFillMode: "forwards",
    animationName: revealLeft,
    animationTimingFunction: "ease-out",
    whiteSpace: "nowrap",
  },
  shell: {
    alignItems: "center",
    display: "flex",
    maxWidth: "100%",
    overflow: "hidden",
    position: "relative",
  },
  state: {
    alignItems: "center",
    display: "flex",
    justifyContent: "flex-start",
    width: "100%",
  },
  stateClipped: {
    overflow: "hidden",
  },
  titleHeight: {
    height: "2rem",
  },
  titleText: {
    fontSize: "1.25rem",
    fontWeight: 600,
  },
});

export { styles as titleInputStyles };
