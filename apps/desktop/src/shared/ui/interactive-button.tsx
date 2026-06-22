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

const DOUBLE_CLICK_DELAY_MS = 350;

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

  useMountEffect(() => {
    return clearPendingClick;
  });

  const handleClick = useCallback(
    (e: MouseEvent<HTMLElement>) => {
      if (disabled) {
        return;
      }

      if (e.shiftKey) {
        e.preventDefault();
        clearPendingClick();
        onShiftClick?.();
      } else if (e.metaKey || e.ctrlKey) {
        e.preventDefault();
        clearPendingClick();
        onCmdClick?.();
      } else if (onDoubleClick) {
        clearPendingClick();
        if (e.detail > 1) {
          return;
        }

        clickTimeoutRef.current = setTimeout(() => {
          clickTimeoutRef.current = null;
          onClick?.();
        }, DOUBLE_CLICK_DELAY_MS);
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

      e.preventDefault();
      clearPendingClick();
      onDoubleClick?.();
    },
    [onDoubleClick, disabled, clearPendingClick],
  );

  const handleDragStart = useCallback(
    (e: DragEvent<HTMLElement>) => {
      clearPendingClick();
      onDragStart?.(e);
    },
    [onDragStart, clearPendingClick],
  );

  const handleContextMenu = useCallback(
    (e: MouseEvent<HTMLElement>) => {
      clearPendingClick();
      showMenu(e);
    },
    [showMenu, clearPendingClick],
  );

  const Element = asChild ? "div" : "button";

  return (
    <Element
      onClick={handleClick}
      onDoubleClick={onDoubleClick ? handleDoubleClick : undefined}
      onDragStart={onDragStart ? handleDragStart : undefined}
      onMouseDown={onMouseDown}
      onContextMenu={contextMenu ? handleContextMenu : undefined}
      className={className}
      disabled={!asChild ? disabled : undefined}
      draggable={draggable}
    >
      {children}
    </Element>
  );
}
