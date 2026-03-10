import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

import { cn } from "@hypr/utils";

export function ChatTrigger({
  onClick,
  isCaretNearBottom = false,
  showTimeline = false,
}: {
  onClick: () => void;
  isCaretNearBottom?: boolean;
  showTimeline?: boolean;
}) {
  const buttonRef = useRef<HTMLButtonElement>(null);
  const [isMouseNearButton, setIsMouseNearButton] = useState(false);

  useEffect(() => {
    if (!isCaretNearBottom) {
      setIsMouseNearButton(false);
      return;
    }

    const handleMouseMove = (e: MouseEvent) => {
      if (!buttonRef.current) return;
      const rect = buttonRef.current.getBoundingClientRect();
      const threshold = 60;
      const isNear =
        e.clientX >= rect.left - threshold &&
        e.clientX <= rect.right + threshold &&
        e.clientY >= rect.top - threshold &&
        e.clientY <= rect.bottom + threshold;
      setIsMouseNearButton(isNear);
    };

    window.addEventListener("mousemove", handleMouseMove);
    return () => window.removeEventListener("mousemove", handleMouseMove);
  }, [isCaretNearBottom]);

  const shouldHide = isCaretNearBottom && !isMouseNearButton;

  return createPortal(
    <button
      ref={buttonRef}
      data-chat-trigger
      onClick={onClick}
      className={cn([
        "fixed right-4 z-40",
        "flex h-14 flex-row items-center justify-center gap-1 rounded-full px-4",
        "bg-white shadow-lg hover:shadow-xl",
        "border border-neutral-200",
        "transition-all duration-200 ease-out",
        "hover:scale-105",
        shouldHide
          ? "bottom-0 translate-y-[85%]"
          : showTimeline
            ? "bottom-[68px]"
            : "bottom-4",
      ])}
    >
      <img
        src="/assets/dynamic.gif"
        alt="Chat Assistant"
        className="size-8 object-contain"
      />
      <span className="text-md font-medium">Chat with notes</span>
    </button>,
    document.body,
  );
}
