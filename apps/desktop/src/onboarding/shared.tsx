import {
  CheckCircle2Icon,
  CheckIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  Loader2Icon,
  XCircleIcon,
} from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { type ReactNode, useEffect, useRef } from "react";

import { cn } from "@hypr/utils";

const SCROLL_DELAY_MS = 350;

export type SectionStatus = "completed" | "active" | "upcoming";

export function OnboardingSection({
  title,
  completedTitle,
  description,
  status,
  onBack,
  onNext,
  children,
}: {
  title: string;
  completedTitle?: string;
  description?: string;
  status: SectionStatus | null;
  onBack?: () => void;
  onNext?: () => void;
  children: ReactNode;
}) {
  const sectionRef = useRef<HTMLElement>(null);

  const isActive = status === "active";
  const isCompleted = status === "completed";

  useEffect(() => {
    if (!isActive) return;
    const timeout = setTimeout(() => {
      sectionRef.current?.scrollIntoView({
        behavior: "smooth",
        block: "start",
      });
    }, SCROLL_DELAY_MS);
    return () => clearTimeout(timeout);
  }, [isActive]);

  if (!status || status === "upcoming") return null;

  return (
    <section ref={sectionRef}>
      <div
        className={cn([
          "flex items-center gap-2 transition-all duration-300",
          isActive && "mb-4",
        ])}
      >
        {isCompleted && (
          <CheckIcon
            className="size-4 shrink-0 text-green-600"
            strokeWidth={2.5}
          />
        )}
        <div className="flex min-w-0 flex-col gap-1">
          <div className="flex items-center gap-2">
            <h2
              className={cn([
                "transition-all duration-300",
                isCompleted
                  ? "text-sm font-normal text-neutral-300"
                  : "font-serif text-lg font-semibold text-neutral-900",
              ])}
            >
              {isCompleted ? (completedTitle ?? title) : title}
            </h2>
            {import.meta.env.DEV && isActive && (onBack || onNext) && (
              <div className="flex items-center gap-2">
                {onBack && (
                  <button
                    onClick={onBack}
                    aria-label="Go to previous section"
                    className="rounded p-0.5 text-neutral-400 transition-colors hover:text-neutral-600"
                  >
                    <ChevronLeftIcon className="size-3" />
                  </button>
                )}
                {onNext && (
                  <button
                    onClick={onNext}
                    aria-label="Go to next section"
                    className="rounded p-0.5 text-neutral-400 transition-colors hover:text-neutral-600"
                  >
                    <ChevronRightIcon className="size-3" />
                  </button>
                )}
              </div>
            )}
          </div>
          {isActive && description && (
            <p className="text-sm text-neutral-500">{description}</p>
          )}
        </div>
      </div>

      <AnimatePresence initial={false}>
        {isActive && (
          <motion.div
            key="content"
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.3, ease: "easeInOut" }}
            className="-mx-5 -mb-5 overflow-hidden px-5 pb-5"
          >
            {children}
          </motion.div>
        )}
      </AnimatePresence>
    </section>
  );
}

export function OnboardingButton(
  props: React.ButtonHTMLAttributes<HTMLButtonElement>,
) {
  return (
    <button
      {...props}
      className="w-fit rounded-full border-2 border-stone-600 bg-stone-800 px-6 py-2.5 text-sm font-medium text-white shadow-[0_4px_14px_rgba(87,83,78,0.4)] transition-all duration-200 hover:bg-stone-700"
    />
  );
}

export function Divider({ text }: { text: string }) {
  return (
    <div className="flex items-center gap-3">
      <div className="h-px flex-1 bg-neutral-200" />
      <span className="text-sm text-neutral-500">{text}</span>
      <div className="h-px flex-1 bg-neutral-200" />
    </div>
  );
}

export function StepRow({
  status,
  label,
}: {
  status: "done" | "active" | "failed";
  label: string;
}) {
  return (
    <div className="flex items-center gap-2 text-sm">
      {status === "done" && (
        <CheckCircle2Icon className="size-4 text-emerald-600" />
      )}
      {status === "active" && (
        <Loader2Icon className="size-4 animate-spin text-neutral-400" />
      )}
      {status === "failed" && <XCircleIcon className="size-4 text-red-400" />}
      <span
        className={status === "failed" ? "text-red-500" : "text-neutral-500"}
      >
        {label}
      </span>
    </div>
  );
}
