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

import { cn } from "@hypr/utils";

import { useTitleGenerating } from "~/ai/hooks";
import * as main from "~/store/tinybase/store/main";
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
    const storeTitle = main.UI.useCell(
      "sessions",
      sessionId,
      "title",
      main.STORE_ID,
    ) as string | undefined;

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
          className={cn([
            "flex w-full items-center justify-start",
            variant === "breadcrumb" ? "h-5" : "h-8",
          ])}
        >
          <span
            className={cn([
              "text-muted-foreground animate-pulse",
              variant === "breadcrumb"
                ? "text-sm leading-5"
                : "text-xl font-semibold",
            ])}
          >
            Generating title...
          </span>
        </div>
      );
    }

    if (showRevealAnimation && generatedTitle) {
      return (
        <div
          data-tauri-drag-region="false"
          className={cn([
            "flex w-full items-center justify-start overflow-hidden",
            variant === "breadcrumb" ? "h-5" : "h-8",
          ])}
        >
          <span
            className={cn([
              "animate-reveal-left whitespace-nowrap",
              variant === "breadcrumb"
                ? "text-sm leading-5"
                : "text-xl font-semibold",
            ])}
          >
            {generatedTitle}
          </span>
        </div>
      );
    }

    return (
      <TitleInputInner
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
        onTransferContentToEditor,
        onFocusEditorAtStart,
        onFocusEditorAtPixelWidth,
        variant,
      },
      ref,
    ) => {
      const storeTitle = main.UI.useCell(
        "sessions",
        sessionId,
        "title",
        main.STORE_ID,
      ) as string | undefined;
      const [draftTitle, setDraftTitle] = useState<string | null>(null);
      const [isOverflowing, setIsOverflowing] = useState(false);
      const [overflowDistance, setOverflowDistance] = useState(0);
      const [showStartFade, setShowStartFade] = useState(false);
      const [showEndFade, setShowEndFade] = useState(false);
      const [isTitleFocused, setIsTitleFocused] = useState(false);
      const internalRef = useRef<HTMLInputElement>(null);
      const setLiveTitle = useLiveTitle((s) => s.setTitle);
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
            "--title-hover-scroll-distance": `-${Math.ceil(
              overflowDistance,
            )}px`,
            "--title-hover-scroll-duration": `${Math.min(
              Math.max(overflowDistance / 48, 2.5),
              8,
            ).toFixed(2)}s`,
          } as CSSProperties)
        : undefined;
      const visibleTitleLength = Math.max(
        title.length || "Untitled".length,
        "Untitled".length,
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

      const setStoreTitle = main.UI.useSetPartialRowCallback(
        "sessions",
        sessionId,
        (title: string) => ({ title }),
        [],
        main.STORE_ID,
      );

      const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
        if (e.key === "ArrowUp") {
          e.preventDefault();
          return;
        }

        if (e.key === "Enter") {
          e.preventDefault();
          const input = internalRef.current;
          if (!input) return;

          const cursorPos = input.selectionStart ?? input.value.length;
          const beforeCursor = input.value.slice(0, cursorPos);
          const afterCursor = input.value.slice(cursorPos);

          setDraftTitle(beforeCursor);
          setStoreTitle(beforeCursor);
          clearLiveTitle(sessionId);

          if (afterCursor) {
            setTimeout(() => onTransferContentToEditor?.(afterCursor), 0);
          } else {
            setTimeout(() => onFocusEditorAtStart?.(), 0);
          }
        } else if (e.key === "Tab") {
          e.preventDefault();
          setTimeout(() => onFocusEditorAtStart?.(), 0);
        } else if (e.key === "ArrowRight") {
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
        } else if (e.key === "ArrowDown") {
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
          style={titleShellStyle}
          className={cn([
            "group/title-input relative flex max-w-full items-center overflow-hidden",
            variant === "breadcrumb"
              ? "h-5 text-sm leading-5"
              : "h-8 text-xl font-semibold",
          ])}
        >
          <input
            data-tauri-drag-region="false"
            aria-label="Session title"
            ref={setInputRef}
            id={`title-input-${sessionId}-${editorId}`}
            placeholder="Untitled"
            type="text"
            onChange={(e) => {
              const value = e.target.value;
              setDraftTitle(value);
              setLiveTitle(sessionId, value);
              updateOverflowState(e.target);
            }}
            onClick={(e) => updateOverflowState(e.currentTarget)}
            onKeyDown={handleKeyDown}
            onKeyUp={(e) => updateOverflowState(e.currentTarget)}
            onFocus={() => {
              setDraftTitle(title);
              setIsTitleFocused(true);
              updateOverflowState();
            }}
            onBlur={(e) => {
              setIsTitleFocused(false);
              setStoreTitle(e.target.value);
              setDraftTitle(null);
              clearLiveTitle(sessionId);
              updateOverflowState(e.target);
            }}
            onScroll={(e) => updateOverflowState(e.currentTarget)}
            onSelect={(e) => updateOverflowState(e.currentTarget)}
            value={title}
            size={visibleTitleLength}
            className={cn([
              "w-full min-w-0 transition-opacity duration-200",
              "border-none bg-transparent focus:outline-hidden",
              "placeholder:text-muted-foreground text-left",
              variant === "breadcrumb"
                ? "h-5 appearance-none p-0 text-sm leading-5 text-neutral-700 focus:underline"
                : "text-xl font-semibold",
              variant === "breadcrumb" &&
                (isTitleFocused
                  ? "overflow-x-auto whitespace-nowrap"
                  : "truncate"),
              showHoverReveal && "text-transparent caret-transparent",
            ])}
          />
          {showHoverReveal ? (
            <div
              aria-hidden="true"
              className="pointer-events-none absolute inset-0 flex items-center justify-start overflow-hidden"
            >
              <span
                style={titleHoverScrollStyle}
                className={cn([
                  "group-hover/title-input:animate-title-hover-scroll whitespace-nowrap group-hover/title-input:will-change-transform",
                  variant === "breadcrumb"
                    ? "text-sm leading-5"
                    : "text-xl font-semibold",
                ])}
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
