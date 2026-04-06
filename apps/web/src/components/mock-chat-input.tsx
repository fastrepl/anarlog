import { Icon } from "@iconify-icon/react";
import { useEffect, useRef, useState } from "react";

import { cn } from "@hypr/utils";

const DEFAULT_PROMPTS = [
  "What are my action items from that meeting?",
  "Summarize the key decisions we made today",
  "What did Sarah say about the project timeline?",
  "List all tasks assigned to me this week",
  "What were the main blockers discussed?",
];

export function MockChatInput({
  prompts = DEFAULT_PROMPTS,
  typingSpeed = 40,
  pauseBetween = 2000,
  className,
}: {
  prompts?: string[];
  typingSpeed?: number;
  pauseBetween?: number;
  className?: string;
}) {
  const [displayText, setDisplayText] = useState("");
  const [promptIndex, setPromptIndex] = useState(0);
  const [isTyping, setIsTyping] = useState(true);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    let charIndex = 0;
    const currentPrompt = prompts[promptIndex];

    const typeNext = () => {
      if (charIndex < currentPrompt.length) {
        charIndex++;
        setDisplayText(currentPrompt.slice(0, charIndex));
        timeoutRef.current = setTimeout(typeNext, typingSpeed);
      } else {
        setIsTyping(false);
        timeoutRef.current = setTimeout(() => {
          setDisplayText("");
          setIsTyping(true);
          setPromptIndex((prev) => (prev + 1) % prompts.length);
        }, pauseBetween);
      }
    };

    setIsTyping(true);
    timeoutRef.current = setTimeout(typeNext, 400);

    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, [promptIndex, prompts, typingSpeed, pauseBetween]);

  return (
    <div
      className={cn([
        "border-color-brand surface flex flex-col overflow-hidden border shadow-lg transition-all duration-300 hover:shadow-2xl",
        "max-h-32 min-h-32 w-full max-w-lg rounded-xl p-3",
        className,
      ])}
    >
      <div className="text-md flex min-h-[24px] flex-1 justify-start pt-2 pl-2 text-stone-600">
        {displayText}
        {isTyping && (
          <span className="ml-[1px] inline-block h-[24px] w-[2px] animate-pulse bg-blue-400 align-middle" />
        )}
      </div>

      <div className="flex w-full justify-end">
        <div
          className={cn([
            "flex size-8 shrink-0 items-center justify-center rounded-lg transition-colors",
            displayText
              ? "bg-stone-600 text-white"
              : "bg-neutral-100 text-neutral-300",
          ])}
        >
          <Icon icon="mdi:arrow-up" className="text-base" />
        </div>
      </div>
    </div>
  );
}
