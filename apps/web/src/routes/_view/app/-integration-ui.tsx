import { Icon } from "@iconify-icon/react";
import type { ButtonHTMLAttributes, ReactNode } from "react";

import { PuzzlePiece } from "@anlg/ui/components/icons";
import { OutlookIcon } from "@anlg/ui/components/icons/outlook";
import { cn } from "@anlg/utils";

const BUTTON_BASE =
  "flex h-12 w-full items-center justify-center gap-2 rounded-full text-base font-medium transition-all";

const BUTTON_VARIANTS = {
  primary: "bg-fg text-white shadow-md hover:bg-fg/90",
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
    <div className="flex min-h-screen items-center justify-center p-6">
      <div className="flex w-full max-w-md flex-col gap-8 text-center">
        {children}
      </div>
    </div>
  );
}

export function integrationIcon(integrationId: string, size = 20): ReactNode {
  switch (integrationId) {
    case "google-calendar":
      return <Icon icon="logos:google-calendar" width={size} height={size} />;
    case "outlook":
      return <OutlookIcon size={size} />;
    case "linear":
      return <Icon icon="logos:linear-icon" width={size} height={size} />;
    case "github":
      return <Icon icon="logos:github-icon" width={size} height={size} />;
    case "slack":
      return <Icon icon="logos:slack-icon" width={size} height={size} />;
    case "notion":
      return <Icon icon="logos:notion-icon" width={size} height={size} />;
    case "zoom":
      return <Icon icon="logos:zoom-icon" width={size} height={size} />;
    case "fathom":
      return <Icon icon="simple-icons:fathom" width={size} height={size} />;
    case "webex":
      return <Icon icon="simple-icons:cisco" width={size} height={size} />;
    case "google-meet":
      return <Icon icon="logos:google-meet" width={size} height={size} />;
    case "microsoft-teams":
      return <Icon icon="logos:microsoft-teams" width={size} height={size} />;
    default:
      return <PuzzlePiece size={size} className="text-color-muted" />;
  }
}
