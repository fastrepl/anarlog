import type { ButtonHTMLAttributes, ReactNode } from "react";

import { cn } from "@hypr/utils";

const BUTTON_BASE =
  "flex h-12 w-full items-center justify-center gap-2 rounded-full text-base font-medium transition-all";

const BUTTON_VARIANTS = {
  primary: "bg-linear-to-t from-stone-600 to-stone-500 text-white shadow-md",
  danger: "bg-linear-to-t from-red-600 to-red-500 text-white shadow-md",
  secondary:
    "border border-neutral-300 bg-linear-to-b from-white to-stone-50 text-neutral-700 shadow-xs",
} as const;

const BUTTON_INTERACTIVE =
  "cursor-pointer hover:scale-[102%] hover:shadow-lg active:scale-[98%]";

const BUTTON_DISABLED =
  "disabled:cursor-not-allowed disabled:pointer-events-none disabled:opacity-70";

export function integrationButtonClassName(
  variant: keyof typeof BUTTON_VARIANTS,
) {
  return cn([
    BUTTON_BASE,
    BUTTON_VARIANTS[variant],
    BUTTON_INTERACTIVE,
    BUTTON_DISABLED,
  ]);
}

export function IntegrationButton({
  variant = "primary",
  className,
  ...props
}: {
  variant?: keyof typeof BUTTON_VARIANTS;
  className?: string;
} & ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      className={cn([integrationButtonClassName(variant), className])}
      {...props}
    />
  );
}

export function IntegrationPageLayout({ children }: { children: ReactNode }) {
  return (
    <div className="flex min-h-screen items-center justify-center bg-linear-to-b from-white via-stone-50/20 to-white p-6">
      <div className="flex w-full max-w-md flex-col gap-8 text-center">
        {children}
      </div>
    </div>
  );
}
