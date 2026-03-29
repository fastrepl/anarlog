import {
  BriefcaseBusinessIcon,
  Code2Icon,
  HandshakeIcon,
  LandmarkIcon,
  MegaphoneIcon,
  PaletteIcon,
  SearchIcon,
  Settings2Icon,
  ShieldCheckIcon,
  TagIcon,
  TrendingUpIcon,
  UsersIcon,
  type LucideIcon,
} from "lucide-react";

import { cn } from "@hypr/utils";

const CATEGORY_ICONS: Record<string, LucideIcon> = {
  "customer success": HandshakeIcon,
  design: PaletteIcon,
  engineering: Code2Icon,
  finance: LandmarkIcon,
  leadership: BriefcaseBusinessIcon,
  legal: ShieldCheckIcon,
  marketing: MegaphoneIcon,
  operations: Settings2Icon,
  people: UsersIcon,
  product: SearchIcon,
  research: SearchIcon,
  sales: TrendingUpIcon,
  support: HandshakeIcon,
};

function getCategoryIcon(category: string) {
  return CATEGORY_ICONS[category.trim().toLowerCase()] ?? TagIcon;
}

export function TemplateCategoryLabel({
  category,
  className,
}: {
  category?: string | null;
  className?: string;
}) {
  if (!category) {
    return null;
  }

  const Icon = getCategoryIcon(category);

  return (
    <span
      className={cn([
        "flex items-center gap-1.5 font-mono text-xs text-stone-400",
        className,
      ])}
    >
      <Icon className="size-3 shrink-0" />
      <span className="truncate">{category}</span>
    </span>
  );
}
