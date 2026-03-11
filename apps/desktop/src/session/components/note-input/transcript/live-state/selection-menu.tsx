import {
  flip,
  FloatingPortal,
  offset,
  shift,
  useFloating,
} from "@floating-ui/react";
import {
  type ChangeEvent,
  type KeyboardEvent,
  type MouseEvent,
  useCallback,
  useEffect,
  useState,
} from "react";

import { cn } from "@hypr/utils";

import { useAutoCloser } from "~/shared/hooks/useAutoCloser";

const MENU_CONTAINER_CLASSES = [
  "pointer-events-auto",
  "flex gap-1",
  "bg-white shadow-lg rounded-md border border-neutral-200 p-1",
];

const MENU_BUTTON_CLASSES = [
  "px-2 py-1 text-xs rounded-xs",
  "hover:bg-neutral-100 transition-colors",
];

const INPUT_CLASSES = [
  "px-2 py-1 text-xs rounded-xs",
  "border border-neutral-300 focus:outline-hidden focus:border-neutral-400",
  "min-w-[120px]",
];

export function SelectionMenu({
  containerRef,
  onAction,
}: {
  containerRef: React.RefObject<HTMLElement | null>;
  onAction?: (
    action: string,
    selectedText: string,
    replaceWith?: string,
  ) => void;
}) {
  const { isVisible, selectedText, hide, refs, floatingStyles, storedRange } =
    useSelectionMenuState({ containerRef });

  const [mode, setMode] = useState<"menu" | "replace">("menu");

  const handleClose = useCallback(() => {
    hide();
    window.getSelection()?.removeAllRanges();
    setMode("menu");
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

  const handleAction = useCallback(
    (action: string, replaceWith?: string) => {
      onAction?.(action, selectedText, replaceWith);
      handleClose();
    },
    [handleClose, onAction, selectedText],
  );

  const handleMouseDown = useCallback((event: MouseEvent<HTMLDivElement>) => {
    event.preventDefault();
  }, []);

  useEffect(() => {
    if (!isVisible) {
      setMode("menu");
    }
  }, [isVisible]);

  if (!isVisible) {
    return null;
  }

  return (
    <>
      <SelectionHighlight range={storedRange} containerRef={containerRef} />
      <FloatingPortal>
        <div
          ref={floatingRef}
          style={{ ...floatingStyles, zIndex: 50 }}
          className={cn(MENU_CONTAINER_CLASSES)}
          onMouseDown={handleMouseDown}
        >
          {mode === "menu" ? (
            <MainMenu
              onRemove={() => handleAction("remove")}
              onReplace={() => setMode("replace")}
            />
          ) : (
            <ReplaceMenu
              onSubmit={(text) => handleAction("replace", text)}
              onCancel={() => setMode("menu")}
            />
          )}
        </div>
      </FloatingPortal>
    </>
  );
}

function MainMenu({
  onRemove,
  onReplace,
}: {
  onRemove: () => void;
  onReplace: () => void;
}) {
  return (
    <>
      <button onClick={onRemove} className={cn(MENU_BUTTON_CLASSES)}>
        Remove
      </button>
      <button onClick={onReplace} className={cn(MENU_BUTTON_CLASSES)}>
        Replace
      </button>
    </>
  );
}

function ReplaceMenu({
  onSubmit,
  onCancel,
}: {
  onSubmit: (text: string) => void;
  onCancel: () => void;
}) {
  const [replaceText, setReplaceText] = useState("");

  const handleSubmit = useCallback(() => {
    onSubmit(replaceText);
  }, [onSubmit, replaceText]);

  const handleChange = useCallback((event: ChangeEvent<HTMLInputElement>) => {
    setReplaceText(event.target.value);
  }, []);

  const handleKeyDown = useCallback(
    (event: KeyboardEvent<HTMLInputElement>) => {
      if (event.key === "Enter") {
        handleSubmit();
      } else if (event.key === "Escape") {
        onCancel();
      }
    },
    [handleSubmit, onCancel],
  );

  return (
    <>
      <input
        type="text"
        value={replaceText}
        onChange={handleChange}
        onKeyDown={handleKeyDown}
        placeholder="Replace with..."
        autoFocus
        className={cn(INPUT_CLASSES)}
      />
      <button onClick={handleSubmit} className={cn(MENU_BUTTON_CLASSES)}>
        ✓
      </button>
      <button onClick={onCancel} className={cn(MENU_BUTTON_CLASSES)}>
        ✕
      </button>
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

    const clientRects = Array.from(range.getClientRects());
    setRects(clientRects);
  }, [range]);

  useEffect(() => {
    if (!range) {
      setRects([]);
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
  }, [range, containerRef, updateRects]);

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
            backgroundColor: "rgba(59, 130, 246, 0.3)",
            pointerEvents: "none",
            zIndex: 40,
          }}
        />
      ))}
    </FloatingPortal>
  );
}

function useSelectionListener({
  containerRef,
  show,
  hide,
  isVisible,
}: {
  containerRef: React.RefObject<HTMLElement | null>;
  show: (text: string, range: Range) => void;
  hide: () => void;
  isVisible: boolean;
}) {
  useEffect(() => {
    const handleSelectionChange = () => {
      const selection = window.getSelection();
      if (!selection || selection.rangeCount === 0) {
        if (!isVisible) {
          hide();
        }
        return;
      }

      const range = selection.getRangeAt(0);
      const trimmedText = selection.toString().trim();

      if (!trimmedText) {
        if (!isVisible) {
          hide();
        }
        return;
      }

      const container = containerRef.current;
      if (!container || !container.contains(range.commonAncestorContainer)) {
        hide();
        return;
      }

      show(trimmedText, range);
    };

    document.addEventListener("selectionchange", handleSelectionChange);

    return () => {
      document.removeEventListener("selectionchange", handleSelectionChange);
    };
  }, [containerRef, hide, show, isVisible]);
}

function useScrollUpdate({
  containerRef,
  isVisible,
  update,
}: {
  containerRef: React.RefObject<HTMLElement | null>;
  isVisible: boolean;
  update: () => void;
}) {
  useEffect(() => {
    if (!isVisible || !containerRef.current) {
      return;
    }

    const container = containerRef.current;
    const handleScroll = () => {
      update();
    };

    container.addEventListener("scroll", handleScroll, { passive: true });

    return () => {
      container.removeEventListener("scroll", handleScroll);
    };
  }, [containerRef, isVisible, update]);
}

function useSelectionMenuState({
  containerRef,
}: {
  containerRef: React.RefObject<HTMLElement | null>;
}) {
  const [isVisible, setIsVisible] = useState(false);
  const [selectedText, setSelectedText] = useState("");
  const [storedRange, setStoredRange] = useState<Range | null>(null);

  const { refs, floatingStyles, update } = useFloating<HTMLElement>({
    open: isVisible,
    placement: "bottom",
    strategy: "fixed",
    transform: false,
    middleware: [offset(6), flip(), shift({ padding: 8 })],
  });

  const show = useCallback(
    (text: string, range: Range) => {
      setSelectedText(text);
      setStoredRange(range.cloneRange());
      setIsVisible(true);
      refs.setPositionReference({
        getBoundingClientRect: () => range.getBoundingClientRect(),
      });
    },
    [refs],
  );

  const hide = useCallback(() => {
    setIsVisible(false);
    setStoredRange(null);
  }, []);

  useSelectionListener({ containerRef, show, hide, isVisible });
  useScrollUpdate({ containerRef, isVisible, update });

  return { isVisible, selectedText, hide, refs, floatingStyles, storedRange };
}
