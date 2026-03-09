import { Icon } from "@iconify-icon/react";

import { cn } from "@hypr/utils";

const CALLOUT_STYLES = {
  note: {
    container: ["bg-blue-50/50", "border-blue-200"],
    icon: "mdi:information-outline",
    iconColor: "text-blue-600",
  },
  warning: {
    container: ["bg-amber-50/50", "border-amber-200"],
    icon: "mdi:alert-outline",
    iconColor: "text-amber-600",
  },
  info: {
    container: ["bg-stone-50/50", "border-neutral-200"],
    icon: "mdi:lightbulb-outline",
    iconColor: "text-stone-600",
  },
  tip: {
    container: ["bg-emerald-50/50", "border-emerald-200"],
    icon: "mdi:check-circle-outline",
    iconColor: "text-emerald-600",
  },
  danger: {
    container: ["bg-red-50/50", "border-red-200"],
    icon: "mdi:alert-circle-outline",
    iconColor: "text-red-600",
  },
} as const;

export function Callout({
  type = "note",
  children,
}: {
  type?: keyof typeof CALLOUT_STYLES;
  children: React.ReactNode;
}) {
  const style = CALLOUT_STYLES[type];

  return (
    <div
      className={cn([
        "not-prose my-6 flex gap-3 rounded-xs border p-4",
        ...style.container,
      ])}
    >
      <Icon
        icon={style.icon}
        className={cn(["shrink-0 text-xl", style.iconColor])}
      />
      <div className="flex-1 text-sm leading-relaxed text-stone-700">
        {children}
      </div>
    </div>
  );
}
