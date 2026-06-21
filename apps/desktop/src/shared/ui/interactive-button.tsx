import {
  type DragEvent,
  type MouseEvent,
  type ReactNode,
  useCallback,
  useRef,
} from "react";

import { useMountEffect } from "~/shared/hooks/useMountEffect";
import {
  type MenuItemDef,
  useNativeContextMenu,
} from "~/shared/hooks/useNativeContextMenu";

interface InteractiveButtonProps {
  children: ReactNode;
  onClick?: () => void;
  onDoubleClick?: () => void;
  onCmdClick?: () => void;
  onShiftClick?: () => void;
  onMouseDown?: (e: MouseEvent<HTMLElement>) => void;
  onDragStart?: (e: DragEvent<HTMLElement>) => void;
  contextMenu?: MenuItemDef[];
  className?: string;
  disabled?: boolean;
  draggable?: boolean;
  asChild?: boolean;
}

export function InteractiveButton({
  children,
  onClick,
  onDoubleClick,
  onCmdClick,
  onShiftClick,
  onMouseDown,
  onDragStart,
  contextMenu,
  className,
  disabled,
  draggable,
  asChild = false,
}: InteractiveButtonProps) {
  const showMenu = useNativeContextMenu(contextMenu ?? []);
  const clickTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const clearPendingClick = useCallback(() => {
    if (clickTimeoutRef.current) {
      clearTimeout(clickTimeoutRef.current);
      clickTimeoutRef.current = null;
    }
  }, []);

  useMountEffect(() => clearPendingClick);

  const handleClick = useCallback(
    (e: MouseEvent<HTMLElement>) => {
      if (disabled) {
        return;
      }

      if (e.shiftKey) {
        clearPendingClick();
        e.preventDefault();
        onShiftClick?.();
      } else if (e.metaKey || e.ctrlKey) {
        clearPendingClick();
        e.preventDefault();
        onCmdClick?.();
      } else if (onDoubleClick) {
        clearPendingClick();
        clickTimeoutRef.current = setTimeout(() => {
          clickTimeoutRef.current = null;
          onClick?.();
        }, 200);
      } else {
        onClick?.();
      }
    },
    [
      onClick,
      onDoubleClick,
      onCmdClick,
      onShiftClick,
      disabled,
      clearPendingClick,
    ],
  );

  const handleDoubleClick = useCallback(
    (e: MouseEvent<HTMLElement>) => {
      if (disabled) {
        return;
      }

      clearPendingClick();
      e.preventDefault();
      onDoubleClick?.();
    },
    [onDoubleClick, disabled, clearPendingClick],
  );

  const Element = asChild ? "div" : "button";

  return (
    <Element
      onClick={handleClick}
      onDoubleClick={onDoubleClick ? handleDoubleClick : undefined}
      onDragStart={onDragStart}
      onMouseDown={onMouseDown}
      onContextMenu={contextMenu ? showMenu : undefined}
      className={className}
      disabled={!asChild ? disabled : undefined}
      draggable={draggable}
    >
      {children}
    </Element>
  );
}
