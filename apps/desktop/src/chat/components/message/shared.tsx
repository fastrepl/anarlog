import { ChevronRight, Loader2 } from "lucide-react";
import { type ReactNode } from "react";

import { cn } from "@hypr/utils";

export function MessageContainer({
  align = "start",
  children,
}: {
  align?: "start" | "end";
  children: ReactNode;
}) {
  return (
    <div
      className={cn([
        "flex px-3 py-2",
        align === "end" ? "justify-end" : "justify-start",
      ])}
    >
      {children}
    </div>
  );
}

export function MessageBubble({
  variant = "assistant",
  withActionButton,
  children,
}: {
  variant?: "user" | "assistant" | "error" | "loading";
  withActionButton?: boolean;
  children: ReactNode;
}) {
  return (
    <div
      className={cn([
        "select-text-deep text-sm",
        variant === "user" &&
          "rounded-2xl bg-blue-100 px-3 py-1 text-neutral-800",
        variant === "assistant" && "text-neutral-800",
        variant === "loading" && "text-neutral-800",
        variant === "error" &&
          "rounded-2xl border border-red-200 bg-red-50 px-3 py-1 text-red-600",
        withActionButton && "group relative",
      ])}
    >
      {children}
    </div>
  );
}

export function ActionButton({
  onClick,
  variant = "default",
  icon: Icon,
  label,
}: {
  onClick: () => void;
  variant?: "default" | "error";
  icon: React.ComponentType<{ className?: string }>;
  label: string;
}) {
  return (
    <button
      aria-label={label}
      onClick={onClick}
      className={cn([
        "absolute -top-1 -right-1",
        "opacity-0 group-hover:opacity-100",
        "transition-opacity",
        "rounded-full p-1",
        variant === "default" && [
          "bg-neutral-200 hover:bg-neutral-300",
          "text-neutral-600 hover:text-neutral-800",
        ],
        variant === "error" && [
          "bg-red-100 hover:bg-red-200",
          "text-red-600 hover:text-red-800",
        ],
      ])}
    >
      <Icon className="h-3 w-3" />
    </button>
  );
}

export function Disclosure({
  icon,
  title,
  children,
  disabled,
}: {
  icon: ReactNode;
  title: ReactNode;
  children: ReactNode;
  disabled?: boolean;
}) {
  return (
    <details
      className={cn([
        "group my-2 rounded-md border px-2 py-1 transition-colors",
        "cursor-pointer border-neutral-200 hover:border-neutral-300",
      ])}
    >
      <summary
        onClick={(event) => {
          if (disabled) {
            event.preventDefault();
          }
        }}
        className={cn([
          "w-full",
          "text-xs text-neutral-500",
          "list-none select-none marker:hidden",
          "flex items-center gap-2",
          disabled && "cursor-default",
        ])}
      >
        {disabled ? <Loader2 className="h-3 w-3 animate-spin" /> : null}
        {!disabled && icon && <span className="shrink-0">{icon}</span>}
        <span className={cn(["flex-1 truncate", "group-open:font-medium"])}>
          {title}
        </span>
        <ChevronRight className="h-3 w-3 shrink-0 transition-transform group-open:rotate-90" />
      </summary>
      <div className="mt-1 border-t border-neutral-200 px-1 pt-2">
        {children}
      </div>
    </details>
  );
}
