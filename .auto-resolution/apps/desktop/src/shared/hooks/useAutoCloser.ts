import { useCallback, useRef } from "react";
import { useHotkeys } from "react-hotkeys-hook";
import { useOnClickOutside } from "usehooks-ts";

export function useAutoCloser(
  onClose: () => void,
  {
    esc = true,
    outside = true,
  }: {
    esc?: boolean;
    outside?: boolean;
  },
) {
  const ref = useRef<HTMLDivElement | null>(null);

  const handleClose = useCallback(() => {
    onClose();
  }, [onClose]);

  useHotkeys("esc", handleClose, { enabled: esc }, [handleClose]);
  useOnClickOutside(
    ref as React.RefObject<HTMLDivElement>,
    outside ? handleClose : () => {},
  );

  return ref;
}
