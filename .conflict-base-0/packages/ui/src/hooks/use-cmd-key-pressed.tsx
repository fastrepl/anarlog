import { useEffect, useState } from "react";

export function useCmdKeyPressed(): boolean {
  const [isCmdPressed, setIsCmdPressed] = useState(false);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.metaKey || e.key === "Meta") {
        // Only show shortcut hints when Cmd is pressed without Shift,
        // so that Cmd+Shift+5 (macOS screenshot) doesn't trigger them.
        // Also hides if Shift is pressed while Cmd is already held.
        setIsCmdPressed(!e.shiftKey);
      }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      if (!e.metaKey || e.key === "Meta") {
        setIsCmdPressed(false);
      }
      // If Shift is released while Cmd is still held, restore the hint
      if (e.key === "Shift" && e.metaKey) {
        setIsCmdPressed(true);
      }
    };

    const handleBlur = () => {
      setIsCmdPressed(false);
    };

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);
    window.addEventListener("blur", handleBlur);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
      window.removeEventListener("blur", handleBlur);
    };
  }, []);

  return isCmdPressed;
}
