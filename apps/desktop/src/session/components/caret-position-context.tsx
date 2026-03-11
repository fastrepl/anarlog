import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";

import type { TiptapEditor } from "@hypr/tiptap/editor";

interface CaretPositionContextValue {
  isCaretNearBottom: boolean;
  setCaretNearBottom: (value: boolean) => void;
}

const CaretPositionContext = createContext<CaretPositionContextValue | null>(
  null,
);

export function CaretPositionProvider({ children }: { children: ReactNode }) {
  const [isCaretNearBottom, setIsCaretNearBottom] = useState(false);

  const setCaretNearBottom = useCallback((value: boolean) => {
    setIsCaretNearBottom(value);
  }, []);

  const value = useMemo(
    () => ({ isCaretNearBottom, setCaretNearBottom }),
    [isCaretNearBottom, setCaretNearBottom],
  );

  return (
    <CaretPositionContext.Provider value={value}>
      {children}
    </CaretPositionContext.Provider>
  );
}

export function useCaretPosition() {
  return useContext(CaretPositionContext);
}

const BOTTOM_THRESHOLD = 70;

export function useCaretNearBottom({
  editor,
  container,
  enabled,
}: {
  editor: TiptapEditor | null;
  container: HTMLDivElement | null;
  enabled: boolean;
}) {
  const setCaretNearBottom = useCaretPosition()?.setCaretNearBottom;

  useEffect(() => {
    if (!setCaretNearBottom) {
      return;
    }

    if (!editor || !container || !enabled) {
      setCaretNearBottom(false);
      return;
    }

    const checkCaretPosition = () => {
      if (!container || !editor.isFocused) {
        return;
      }

      const { view } = editor;
      const { from } = view.state.selection;
      const coords = view.coordsAtPos(from);

      const distanceFromViewportBottom = window.innerHeight - coords.bottom;

      setCaretNearBottom(distanceFromViewportBottom < BOTTOM_THRESHOLD);
    };

    const handleBlur = () => setCaretNearBottom(false);

    editor.on("selectionUpdate", checkCaretPosition);
    editor.on("focus", checkCaretPosition);
    editor.on("blur", handleBlur);
    container.addEventListener("scroll", checkCaretPosition);
    window.addEventListener("resize", checkCaretPosition);

    checkCaretPosition();

    return () => {
      editor.off("selectionUpdate", checkCaretPosition);
      editor.off("focus", checkCaretPosition);
      editor.off("blur", handleBlur);
      container.removeEventListener("scroll", checkCaretPosition);
      window.removeEventListener("resize", checkCaretPosition);
    };
  }, [editor, setCaretNearBottom, container, enabled]);
}
