// Exports the Hugeicons used by the templates and folders routes into icons/catalog/.
// Template icon keys mirror apps/desktop/src/templates/template-icon.tsx.
import * as hugeicons from "@hugeicons/core-free-icons";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";

export const TEMPLATE_ICONS = {
  activity: "Pulse01Icon",
  "bar-chart": "ChartBarBigIcon",
  bell: "BellIcon",
  "book-open": "BookOpen01Icon",
  brain: "BrainIcon",
  briefcase: "Briefcase01Icon",
  bug: "Bug01Icon",
  building: "Building03Icon",
  calendar: "Calendar03Icon",
  camera: "Camera01Icon",
  chart: "ChartLineData01Icon",
  alert: "AlertCircleIcon",
  "clipboard-check": "Task01Icon",
  clock: "Clock01Icon",
  cloud: "CloudIcon",
  code: "CodeIcon",
  coffee: "Coffee01Icon",
  compass: "CompassIcon",
  "credit-card": "CreditCardIcon",
  crown: "CrownIcon",
  database: "DatabaseIcon",
  dollar: "DollarSignIcon",
  "file-text": "FileTextIcon",
  flag: "Flag01Icon",
  flame: "FireIcon",
  folder: "Folder01Icon",
  globe: "GlobeIcon",
  graduation: "GraduationCapIcon",
  hammer: "HammerIcon",
  handshake: "HandshakeIcon",
  headphones: "HeadphonesIcon",
  heart: "HeartIcon",
  home: "House01Icon",
  image: "Image01Icon",
  key: "Key01Icon",
  landmark: "BankIcon",
  leaf: "Leaf01Icon",
  lightbulb: "LightbulbIcon",
  link: "LinkIcon",
  "list-checks": "ListChecksIcon",
  lock: "LockPasswordIcon",
  mail: "Mail01Icon",
  map: "MapIcon",
  megaphone: "Megaphone01Icon",
  message: "MessageCircleIcon",
  messages: "MessageMultiple01Icon",
  mic: "Mic01Icon",
  milestone: "SignpostIcon",
  moon: "MoonIcon",
  music: "MusicNote01Icon",
  "notebook-tabs": "NotebookIcon",
  package: "PackageIcon",
  palette: "PaletteIcon",
  paperclip: "PaperclipIcon",
  pen: "PencilLineIcon",
  phone: "PhoneIcon",
  "pie-chart": "PieChartIcon",
  plane: "Airplane01Icon",
  presentation: "Presentation01Icon",
  puzzle: "PuzzleIcon",
  rocket: "RocketIcon",
  scale: "BalanceScaleIcon",
  search: "Search01Icon",
  settings: "Settings02Icon",
  shield: "ShieldCheckIcon",
  shopping: "ShoppingBag01Icon",
  sparkles: "SparkleIcon",
  kanban: "KanbanIcon",
  star: "StarIcon",
  stethoscope: "StethoscopeIcon",
  sun: "Sun01Icon",
  target: "Target01Icon",
  trending: "TrendingUpIcon",
  trophy: "TrophyIcon",
  "user-search": "UserSearch01Icon",
  users: "UsersIcon",
  video: "Video01Icon",
  wrench: "Wrench01Icon",
  zap: "ZapIcon",
};

// Interface glyphs: ArrowsDownUp, BookOpenText, DotsSixVertical, X, File, MagicWand, Plus,
// DotsThree, Check, ArrowLeft, ShareNetwork. `weight="bold"` variants stroke at 2.
const INTERFACE = [
  "ArrowUpDownIcon",
  "BookOpenTextIcon",
  "GripVerticalIcon",
  "XIcon",
  "File01Icon",
  "MagicWand01Icon",
  "PlusIcon",
  "MoreHorizontalIcon",
  "CheckIcon",
  "ArrowLeft02Icon",
  "Share08Icon",
];
const BOLD = ["HeartIcon", "CheckIcon"];

const escape = (value) =>
  String(value)
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;");

function svg(data, strokeWidth) {
  const elements = data
    .map(
      ([tag, attrs]) =>
        `<${tag} ${Object.entries(attrs)
          .filter(([key]) => key !== "key")
          .map(([key, value]) => [
            key,
            key === "strokeWidth" ? strokeWidth : value,
          ])
          .map(
            ([key, value]) =>
              `${key.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)}="${escape(value)}"`,
          )
          .join(" ")} />`,
    )
    .join("");
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="${strokeWidth}" stroke-linecap="round" stroke-linejoin="round">${elements}</svg>\n`;
}

const out = new URL("./icons/catalog/", import.meta.url);
mkdirSync(out, { recursive: true });
const names = new Set([...Object.values(TEMPLATE_ICONS), ...INTERFACE]);
for (const name of names) {
  const data = hugeicons[name];
  if (!data) throw new Error(`missing icon ${name}`);
  writeFileSync(new URL(`${name}.svg`, out), svg(data, "1.5"));
}
for (const name of BOLD) {
  writeFileSync(new URL(`${name}Bold.svg`, out), svg(hugeicons[name], "2"));
}

// The emoji picker's table: category, id, native glyph and search text per line, in
// emoji-mart order (`template-icon-picker.tsx`).
const emoji = JSON.parse(
  readFileSync(
    new URL(import.meta.resolve("@emoji-mart/data/sets/15/native.json")),
    "utf8",
  ),
);
const CATEGORIES = [
  "people",
  "nature",
  "foods",
  "activity",
  "places",
  "objects",
  "symbols",
  "flags",
];
const clean = (text) => text.replace(/[\t\n]/g, " ");
const lines = emoji.categories
  .filter((category) => CATEGORIES.includes(category.id))
  .flatMap((category) =>
    category.emojis.flatMap((id) => {
      const item = emoji.emojis[id];
      const native = item?.skins[0]?.native;
      if (!native) return [];
      const search = [item.id, item.name, ...item.keywords]
        .join(" ")
        .toLowerCase();
      return [
        [category.id, id, native, clean(item.name), clean(search)].join("\t"),
      ];
    }),
  );
writeFileSync(new URL("emoji.tsv", out), `${lines.join("\n")}\n`);
