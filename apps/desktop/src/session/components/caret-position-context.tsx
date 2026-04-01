import type { EditorView } from "prosemirror-view";
import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";

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
  view,
  container,
  enabled,
}: {
  view: EditorView | null;
  container: HTMLDivElement | null;
  enabled: boolean;
}) {
  const setCaretNearBottom = useCaretPosition()?.setCaretNearBottom;

  useEffect(() => {
    if (!setCaretNearBottom) {
      return;
    }

    if (!view || !container || !enabled) {
      setCaretNearBottom(false);
      return;
    }

    const checkCaretPosition = () => {
      if (!container || !view.hasFocus()) {
        return;
      }

      const { from } = view.state.selection;
      const coords = view.coordsAtPos(from);

      const distanceFromViewportBottom = window.innerHeight - coords.bottom;

      setCaretNearBottom(distanceFromViewportBottom < BOTTOM_THRESHOLD);
    };

    const handleBlur = () => setCaretNearBottom(false);

    const dom = view.dom;
    dom.addEventListener("focus", checkCaretPosition);
    dom.addEventListener("blur", handleBlur);
    dom.addEventListener("keyup", checkCaretPosition);
    dom.addEventListener("mouseup", checkCaretPosition);
    container.addEventListener("scroll", checkCaretPosition);
    window.addEventListener("resize", checkCaretPosition);

    checkCaretPosition();

    return () => {
      dom.removeEventListener("focus", checkCaretPosition);
      dom.removeEventListener("blur", handleBlur);
      dom.removeEventListener("keyup", checkCaretPosition);
      dom.removeEventListener("mouseup", checkCaretPosition);
      container.removeEventListener("scroll", checkCaretPosition);
      window.removeEventListener("resize", checkCaretPosition);
    };
  }, [view, setCaretNearBottom, container, enabled]);
}
