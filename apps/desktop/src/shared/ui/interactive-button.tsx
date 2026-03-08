import { type MouseEvent, type ReactNode, useCallback } from "react";

import {
  type MenuItemDef,
  useNativeContextMenu,
} from "~/shared/hooks/useNativeContextMenu";

interface InteractiveButtonProps {
  children: ReactNode;
  onClick?: () => void;
  onCmdClick?: () => void;
  onShiftClick?: () => void;
  onMouseDown?: (e: MouseEvent<HTMLElement>) => void;
  contextMenu?: MenuItemDef[];
  className?: string;
  disabled?: boolean;
  asChild?: boolean;
}

export function InteractiveButton({
  children,
  onClick,
  onCmdClick,
  onShiftClick,
  onMouseDown,
  contextMenu,
  className,
  disabled,
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
      } else {
        onClick?.();
      }
    },
    [onClick, onCmdClick, onShiftClick, disabled],
  );

  const Element = asChild ? "div" : "button";

  return (
    <Element
      onClick={handleClick}
      onMouseDown={onMouseDown}
      onContextMenu={contextMenu ? showMenu : undefined}
      className={className}
      disabled={!asChild ? disabled : undefined}
    >
      {children}
    </Element>
  );
}
