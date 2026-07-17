import {
  type DragEvent,
  type MouseEvent,
  type ReactNode,
  useCallback,
} from "react";

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

  const handleClick = useCallback(
    (e: MouseEvent<HTMLElement>) => {
      if (disabled) {
        return;
      }

      if (e.shiftKey) {
        e.preventDefault();
        onShiftClick?.();
      } else if (e.metaKey || e.ctrlKey) {
        e.preventDefault();
        onCmdClick?.();
      } else if (onDoubleClick) {
        if (e.detail > 1) {
          return;
        }

        onClick?.();
      } else {
        onClick?.();
      }
    },
    [onClick, onDoubleClick, onCmdClick, onShiftClick, disabled],
  );

  const handleDoubleClick = useCallback(
    (e: MouseEvent<HTMLElement>) => {
      if (disabled) {
        return;
      }

      e.preventDefault();
      onDoubleClick?.();
    },
    [onDoubleClick, disabled],
  );

  const handleDragStart = useCallback(
    (e: DragEvent<HTMLElement>) => {
      onDragStart?.(e);
    },
    [onDragStart],
  );

  const handleContextMenu = useCallback(
    (e: MouseEvent<HTMLElement>) => {
      showMenu(e);
    },
    [showMenu],
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
