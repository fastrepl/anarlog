import {
  Bank,
  Briefcase,
  Code,
  GearSix,
  Handshake,
  type Icon,
  MagnifyingGlass,
  Megaphone,
  Palette,
  ShieldCheck,
  Tag,
  TrendUp,
  Users,
} from "@phosphor-icons/react";

import { cn } from "@anlg/utils";

const CATEGORY_ICONS: Record<string, Icon> = {
  "customer success": Handshake,
  design: Palette,
  engineering: Code,
  finance: Bank,
  leadership: Briefcase,
  legal: ShieldCheck,
  marketing: Megaphone,
  operations: GearSix,
  people: Users,
  product: MagnifyingGlass,
  research: MagnifyingGlass,
  sales: TrendUp,
  support: Handshake,
};

function getCategoryIcon(category: string) {
  return CATEGORY_ICONS[category.trim().toLowerCase()] ?? Tag;
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
        "text-muted-foreground flex items-center gap-1.5 text-xs",
        className,
      ])}
    >
      <Icon className="size-3 shrink-0" />
      <span className="truncate">{category}</span>
    </span>
  );
}
