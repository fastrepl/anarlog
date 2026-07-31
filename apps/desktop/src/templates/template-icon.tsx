import {
  Airplane,
  Bank,
  Bell,
  BookOpen,
  Brain,
  Briefcase,
  Bug,
  Buildings,
  CalendarDots,
  Camera,
  ChartBar,
  ChartLineUp,
  ChartPie,
  ChatCircle,
  Chats,
  ClipboardText,
  Clock,
  Cloud,
  Code,
  Coffee,
  Compass,
  CreditCard,
  Crown,
  CurrencyDollar,
  Database,
  Envelope,
  FileText,
  Fire,
  Flag,
  Gear,
  Globe,
  GraduationCap,
  Hammer,
  Handshake,
  Headphones,
  Heart,
  House,
  type Icon,
  Image,
  Kanban,
  Key,
  Leaf,
  Lightbulb,
  Lightning,
  Link,
  ListChecks,
  Lock,
  MagnifyingGlass,
  MapTrifold,
  Megaphone,
  Microphone,
  Moon,
  MusicNote,
  Notebook,
  Package,
  Palette,
  Paperclip,
  PencilSimpleLine,
  Phone,
  Presentation,
  Pulse,
  PuzzlePiece,
  Rocket,
  Scales,
  ShieldCheck,
  ShoppingBag,
  Signpost,
  Sparkle,
  Star,
  Stethoscope,
  Sun,
  Target,
  TrendUp,
  Trophy,
  UserFocus,
  Users,
  VideoCamera,
  WarningCircle,
  Wrench,
} from "@phosphor-icons/react";

import { cn } from "@anlg/utils";

export type TemplateIcon =
  | { type: "icon"; value: string; color: string }
  | { type: "emoji"; value: string };

export const DEFAULT_TEMPLATE_ICON = {
  type: "icon",
  value: "notebook-tabs",
  color: "#9ca3af",
} as const satisfies TemplateIcon;

const TEMPLATE_ICON_COMPONENTS: Record<string, Icon> = {
  activity: Pulse,
  "bar-chart": ChartBar,
  bell: Bell,
  "book-open": BookOpen,
  brain: Brain,
  briefcase: Briefcase,
  bug: Bug,
  building: Buildings,
  calendar: CalendarDots,
  camera: Camera,
  chart: ChartLineUp,
  alert: WarningCircle,
  "clipboard-check": ClipboardText,
  clock: Clock,
  cloud: Cloud,
  code: Code,
  coffee: Coffee,
  compass: Compass,
  "credit-card": CreditCard,
  crown: Crown,
  database: Database,
  dollar: CurrencyDollar,
  "file-text": FileText,
  flag: Flag,
  flame: Fire,
  globe: Globe,
  graduation: GraduationCap,
  hammer: Hammer,
  handshake: Handshake,
  headphones: Headphones,
  heart: Heart,
  home: House,
  image: Image,
  key: Key,
  landmark: Bank,
  leaf: Leaf,
  lightbulb: Lightbulb,
  link: Link,
  "list-checks": ListChecks,
  lock: Lock,
  mail: Envelope,
  map: MapTrifold,
  megaphone: Megaphone,
  message: ChatCircle,
  messages: Chats,
  mic: Microphone,
  milestone: Signpost,
  moon: Moon,
  music: MusicNote,
  "notebook-tabs": Notebook,
  package: Package,
  palette: Palette,
  paperclip: Paperclip,
  pen: PencilSimpleLine,
  phone: Phone,
  "pie-chart": ChartPie,
  plane: Airplane,
  presentation: Presentation,
  puzzle: PuzzlePiece,
  rocket: Rocket,
  scale: Scales,
  search: MagnifyingGlass,
  settings: Gear,
  shield: ShieldCheck,
  shopping: ShoppingBag,
  sparkles: Sparkle,
  kanban: Kanban,
  star: Star,
  stethoscope: Stethoscope,
  sun: Sun,
  target: Target,
  trending: TrendUp,
  trophy: Trophy,
  "user-search": UserFocus,
  users: Users,
  video: VideoCamera,
  wrench: Wrench,
  zap: Lightning,
};

export const TEMPLATE_ICONS = Object.entries(TEMPLATE_ICON_COMPONENTS).map(
  ([value, component]) => ({
    value,
    component,
    search: value.replace(/-/g, " "),
  }),
);

function isHexColor(value: unknown): value is string {
  return typeof value === "string" && /^#[0-9a-f]{6}$/i.test(value);
}

export function normalizeTemplateIcon(value: unknown): TemplateIcon {
  let candidate = value;
  if (typeof value === "string") {
    try {
      candidate = JSON.parse(value);
    } catch {
      return DEFAULT_TEMPLATE_ICON;
    }
  }

  if (!candidate || typeof candidate !== "object") {
    return DEFAULT_TEMPLATE_ICON;
  }

  const icon = candidate as Record<string, unknown>;
  if (
    icon.type === "emoji" &&
    typeof icon.value === "string" &&
    icon.value.trim()
  ) {
    return { type: "emoji", value: icon.value };
  }

  if (
    icon.type === "icon" &&
    typeof icon.value === "string" &&
    TEMPLATE_ICON_COMPONENTS[icon.value] &&
    isHexColor(icon.color)
  ) {
    return { type: "icon", value: icon.value, color: icon.color };
  }

  return DEFAULT_TEMPLATE_ICON;
}

export function TemplateIconGlyph({
  icon,
  className,
}: {
  icon: TemplateIcon | unknown;
  className?: string;
}) {
  const normalized = normalizeTemplateIcon(icon);
  if (normalized.type === "emoji") {
    return (
      <span
        aria-hidden
        className={cn([
          "inline-flex shrink-0 items-center justify-center",
          className,
        ])}
      >
        {normalized.value}
      </span>
    );
  }

  const Icon = TEMPLATE_ICON_COMPONENTS[normalized.value] ?? Notebook;
  return (
    <Icon
      aria-hidden
      className={cn(["shrink-0", className])}
      style={{ color: normalized.color }}
    />
  );
}
