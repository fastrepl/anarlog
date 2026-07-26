import {
  Activity,
  BarChart3,
  Bell,
  BookOpen,
  Brain,
  BriefcaseBusiness,
  Bug,
  Building2,
  CalendarDays,
  Camera,
  ChartNoAxesCombined,
  CircleAlert,
  ClipboardCheck,
  Clock3,
  Cloud,
  Code2,
  Coffee,
  Compass,
  CreditCard,
  Crown,
  Database,
  DollarSign,
  FileText,
  Flag,
  Flame,
  Globe2,
  GraduationCap,
  Hammer,
  Handshake,
  Headphones,
  Heart,
  Home,
  Image,
  KeyRound,
  Landmark,
  Leaf,
  Lightbulb,
  Link,
  ListChecks,
  Lock,
  Mail,
  Map,
  Megaphone,
  MessageCircle,
  MessagesSquare,
  Mic,
  Milestone,
  Moon,
  Music2,
  NotebookTabs,
  Package,
  Palette,
  Paperclip,
  PenLine,
  Phone,
  PieChart,
  Plane,
  Presentation,
  Puzzle,
  Rocket,
  Scale,
  Search,
  Settings,
  ShieldCheck,
  ShoppingBag,
  Sparkles,
  SquareKanban,
  Star,
  Stethoscope,
  Sun,
  Target,
  TrendingUp,
  Trophy,
  UserRoundSearch,
  Users,
  Video,
  Wrench,
  Zap,
  type LucideIcon,
} from "lucide-react";

import { cn } from "@hypr/utils";

export type TemplateIcon =
  | { type: "icon"; value: string; color: string }
  | { type: "emoji"; value: string };

export const DEFAULT_TEMPLATE_ICON = {
  type: "icon",
  value: "notebook-tabs",
  color: "#9ca3af",
} as const satisfies TemplateIcon;

const TEMPLATE_ICON_COMPONENTS: Record<string, LucideIcon> = {
  activity: Activity,
  "bar-chart": BarChart3,
  bell: Bell,
  "book-open": BookOpen,
  brain: Brain,
  briefcase: BriefcaseBusiness,
  bug: Bug,
  building: Building2,
  calendar: CalendarDays,
  camera: Camera,
  chart: ChartNoAxesCombined,
  alert: CircleAlert,
  "clipboard-check": ClipboardCheck,
  clock: Clock3,
  cloud: Cloud,
  code: Code2,
  coffee: Coffee,
  compass: Compass,
  "credit-card": CreditCard,
  crown: Crown,
  database: Database,
  dollar: DollarSign,
  "file-text": FileText,
  flag: Flag,
  flame: Flame,
  globe: Globe2,
  graduation: GraduationCap,
  hammer: Hammer,
  handshake: Handshake,
  headphones: Headphones,
  heart: Heart,
  home: Home,
  image: Image,
  key: KeyRound,
  landmark: Landmark,
  leaf: Leaf,
  lightbulb: Lightbulb,
  link: Link,
  "list-checks": ListChecks,
  lock: Lock,
  mail: Mail,
  map: Map,
  megaphone: Megaphone,
  message: MessageCircle,
  messages: MessagesSquare,
  mic: Mic,
  milestone: Milestone,
  moon: Moon,
  music: Music2,
  "notebook-tabs": NotebookTabs,
  package: Package,
  palette: Palette,
  paperclip: Paperclip,
  pen: PenLine,
  phone: Phone,
  "pie-chart": PieChart,
  plane: Plane,
  presentation: Presentation,
  puzzle: Puzzle,
  rocket: Rocket,
  scale: Scale,
  search: Search,
  settings: Settings,
  shield: ShieldCheck,
  shopping: ShoppingBag,
  sparkles: Sparkles,
  kanban: SquareKanban,
  star: Star,
  stethoscope: Stethoscope,
  sun: Sun,
  target: Target,
  trending: TrendingUp,
  trophy: Trophy,
  "user-search": UserRoundSearch,
  users: Users,
  video: Video,
  wrench: Wrench,
  zap: Zap,
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

  const Icon = TEMPLATE_ICON_COMPONENTS[normalized.value] ?? NotebookTabs;
  return (
    <Icon
      aria-hidden
      className={cn(["shrink-0", className])}
      style={{ color: normalized.color }}
    />
  );
}
