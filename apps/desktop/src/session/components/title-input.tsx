import { usePrevious } from "@uidotdev/usehooks";
import { SparklesIcon } from "lucide-react";
import {
  forwardRef,
  memo,
  useCallback,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
} from "react";

import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import { useTitleGenerating } from "~/ai/hooks";
import * as main from "~/store/tinybase/store/main";
import { useLiveTitle } from "~/store/zustand/live-title";
import { type Tab } from "~/store/zustand/tabs";

export const TitleInput = forwardRef<
  HTMLInputElement,
  {
    tab: Extract<Tab, { type: "sessions" }>;
    onNavigateToEditor?: () => void;
    onGenerateTitle?: () => void;
  }
>(({ tab, onNavigateToEditor, onGenerateTitle }, ref) => {
  const {
    id: sessionId,
    state: { view },
  } = tab;
  const store = main.UI.useStore(main.STORE_ID);
  const isGenerating = useTitleGenerating(sessionId);
  const wasGenerating = usePrevious(isGenerating);
  const [showRevealAnimation, setShowRevealAnimation] = useState(false);
  const [generatedTitle, setGeneratedTitle] = useState<string | null>(null);

  const editorId = view ? "active" : "inactive";
  const inputRef = useRef<HTMLInputElement>(null);

  useImperativeHandle(ref, () => inputRef.current!, []);

  useEffect(() => {
    if (wasGenerating && !isGenerating) {
      const title = store?.getCell("sessions", sessionId, "title") as
        | string
        | undefined;
      setGeneratedTitle(title ?? null);
      setShowRevealAnimation(true);
      const timer = setTimeout(() => {
        setShowRevealAnimation(false);
      }, 1000);
      return () => clearTimeout(timer);
    }
  }, [wasGenerating, isGenerating, store, sessionId]);

  const getInitialTitle = useCallback(() => {
    return (store?.getCell("sessions", sessionId, "title") as string) ?? "";
  }, [store, sessionId]);

  if (isGenerating) {
    return (
      <div className="flex h-[28px] w-full items-center">
        <span className="text-muted-foreground animate-pulse text-xl font-semibold">
          Generating title...
        </span>
      </div>
    );
  }

  if (showRevealAnimation && generatedTitle) {
    return (
      <div className="flex h-[28px] w-full items-center overflow-hidden">
        <span className="animate-reveal-left text-xl font-semibold whitespace-nowrap">
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
      getInitialTitle={getInitialTitle}
      onNavigateToEditor={onNavigateToEditor}
      onGenerateTitle={onGenerateTitle}
    />
  );
});

const TitleInputInner = memo(
  forwardRef<
    HTMLInputElement,
    {
      sessionId: string;
      editorId: string;
      getInitialTitle: () => string;
      onNavigateToEditor?: () => void;
      onGenerateTitle?: () => void;
    }
  >(
    (
      {
        sessionId,
        editorId,
        getInitialTitle,
        onNavigateToEditor,
        onGenerateTitle,
      },
      ref,
    ) => {
      const [localTitle, setLocalTitle] = useState(() => getInitialTitle());
      const isFocused = useRef(false);
      const internalRef = useRef<HTMLInputElement>(null);
      const store = main.UI.useStore(main.STORE_ID);
      const setLiveTitle = useLiveTitle((s) => s.setTitle);
      const clearLiveTitle = useLiveTitle((s) => s.clearTitle);

      useImperativeHandle(ref, () => internalRef.current!, []);

      useEffect(() => {
        if (!store) return;

        const listenerId = store.addCellListener(
          "sessions",
          sessionId,
          "title",
          (_store, _tableId, _rowId, _cellId, newValue) => {
            if (!isFocused.current) {
              setLocalTitle((newValue as string) ?? "");
            }
          },
        );

        return () => {
          store.delListener(listenerId);
        };
      }, [store, sessionId]);

      const setStoreTitle = main.UI.useSetPartialRowCallback(
        "sessions",
        sessionId,
        (title: string) => ({ title }),
        [],
        main.STORE_ID,
      );

      useEffect(() => {
        const handleMoveToTitlePosition = (e: Event) => {
          const customEvent = e as CustomEvent<{ pixelWidth: number }>;
          const pixelWidth = customEvent.detail.pixelWidth;
          const input = internalRef.current;

          if (input && input.value) {
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
          }
        };

        window.addEventListener(
          "editor-move-to-title-position",
          handleMoveToTitlePosition,
        );
        return () => {
          window.removeEventListener(
            "editor-move-to-title-position",
            handleMoveToTitlePosition,
          );
        };
      }, []);

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

          setStoreTitle(beforeCursor);
          clearLiveTitle(sessionId);

          if (afterCursor) {
            setTimeout(() => {
              const event = new CustomEvent("title-content-transfer", {
                detail: { content: afterCursor },
              });
              window.dispatchEvent(event);
            }, 0);
          } else {
            setTimeout(() => {
              const event = new CustomEvent("title-move-to-editor-start");
              window.dispatchEvent(event);
            }, 0);
          }

          onNavigateToEditor?.();
        } else if (e.key === "Tab") {
          e.preventDefault();
          setTimeout(() => {
            const event = new CustomEvent("title-move-to-editor-start");
            window.dispatchEvent(event);
          }, 0);
          onNavigateToEditor?.();
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

            setTimeout(() => {
              const event = new CustomEvent("title-move-to-editor-position", {
                detail: { pixelWidth: titleWidth },
              });
              window.dispatchEvent(event);
            }, 0);
          }

          onNavigateToEditor?.();
        }
      };

      return (
        <div className="flex w-full items-center gap-2">
          <input
            ref={internalRef}
            id={`title-input-${sessionId}-${editorId}`}
            placeholder="Untitled"
            type="text"
            onChange={(e) => {
              const value = e.target.value;
              setLocalTitle(value);
              setLiveTitle(sessionId, value);
            }}
            onKeyDown={handleKeyDown}
            onFocus={() => {
              isFocused.current = true;
            }}
            onBlur={() => {
              isFocused.current = false;
              setStoreTitle(localTitle);
              clearLiveTitle(sessionId);
            }}
            value={localTitle}
            className={cn([
              "min-w-0 flex-1 transition-opacity duration-200",
              "border-none bg-transparent focus:outline-hidden",
              "placeholder:text-muted-foreground text-xl font-semibold",
            ])}
          />
          {onGenerateTitle && (
            <GenerateButton onGenerateTitle={onGenerateTitle} />
          )}
        </div>
      );
    },
  ),
);

const GenerateButton = memo(function GenerateButton({
  onGenerateTitle,
}: {
  onGenerateTitle: () => void;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          onClick={(e) => {
            e.preventDefault();
            e.stopPropagation();
            onGenerateTitle();
          }}
          onMouseDown={(e) => e.preventDefault()}
          className={cn([
            "shrink-0",
            "text-muted-foreground hover:text-foreground",
            "opacity-50 transition-opacity hover:opacity-100",
          ])}
        >
          <SparklesIcon className="h-4 w-4" />
        </button>
      </TooltipTrigger>
      <TooltipContent side="bottom">Regenerate title</TooltipContent>
    </Tooltip>
  );
});
