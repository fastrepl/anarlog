import emojiData, { type Emoji, type EmojiMartData } from "@emoji-mart/data";
import { useLingui } from "@lingui/react/macro";
import { Check, Search, X } from "lucide-react";
import { useMemo, useState } from "react";
import { HexColorInput, HexColorPicker } from "react-colorful";

import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import { cn } from "@hypr/utils";

import {
  DEFAULT_TEMPLATE_ICON,
  TEMPLATE_ICONS,
  TemplateIconGlyph,
  normalizeTemplateIcon,
  type TemplateIcon,
} from "./template-icon";

const ICON_COLORS = [
  "#9ca3af",
  "#94a3b8",
  "#5b67d8",
  "#25b5c9",
  "#4ab883",
  "#f2bd00",
  "#ef923d",
  "#c99b92",
  "#f05257",
];

const EMOJI_CATEGORY_IDS = new Set([
  "people",
  "nature",
  "foods",
  "activity",
  "places",
  "objects",
  "symbols",
  "flags",
]);

const FREQUENT_EMOJI_IDS = [
  "ok_hand",
  "heart",
  "white_check_mark",
  "+1",
  "pray",
  "joy",
  "eyes",
  "slightly_smiling_face",
  "grinning",
  "smile",
  "thinking_face",
  "sweat_smile",
  "warning",
  "confused",
  "x",
  "raised_hands",
  "tada",
  "wink",
  "blush",
  "shrug",
  "wave",
  "question",
];

const RECENT_EMOJIS_KEY = "anarlog.template-picker.recent-emojis";
const data = emojiData as EmojiMartData;

type EmojiItem = {
  id: string;
  native: string;
  name: string;
  search: string;
};

function toEmojiItem(emoji: Emoji): EmojiItem | null {
  const native = emoji.skins[0]?.native;
  if (!native) {
    return null;
  }

  return {
    id: emoji.id,
    native,
    name: emoji.name,
    search: [emoji.id, emoji.name, ...emoji.keywords].join(" ").toLowerCase(),
  };
}

const EMOJI_ITEMS = Object.values(data.emojis).reduce<
  Record<string, EmojiItem>
>((items, emoji) => {
  const item = toEmojiItem(emoji);
  if (item) {
    items[emoji.id] = item;
  }
  return items;
}, {});

const EMOJI_CATEGORIES = data.categories.flatMap((category) => {
  if (!EMOJI_CATEGORY_IDS.has(category.id)) {
    return [];
  }

  return [
    {
      id: category.id,
      items: category.emojis.flatMap((id) =>
        EMOJI_ITEMS[id] ? [EMOJI_ITEMS[id]] : [],
      ),
    },
  ];
});

function loadRecentEmojiIds() {
  if (typeof window === "undefined") {
    return [];
  }

  try {
    const value = JSON.parse(
      window.localStorage.getItem(RECENT_EMOJIS_KEY) ?? "[]",
    );
    return Array.isArray(value)
      ? value.filter((item): item is string => typeof item === "string")
      : [];
  } catch {
    return [];
  }
}

function SearchField({
  value,
  onChange,
  placeholder,
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
}) {
  return (
    <div className="border-border flex h-12 items-center gap-2 border-b px-4">
      <Search className="text-muted-foreground size-4 shrink-0" />
      <input
        type="text"
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
        className="placeholder:text-muted-foreground min-w-0 flex-1 bg-transparent text-sm outline-hidden"
      />
      {value ? (
        <button
          type="button"
          onClick={() => onChange("")}
          className="hover:bg-accent rounded-sm p-1"
          aria-label="Clear search"
        >
          <X className="text-muted-foreground size-3.5" />
        </button>
      ) : null}
    </div>
  );
}

export function TemplateIconPicker({
  value,
  onChange,
}: {
  value: TemplateIcon;
  onChange: (value: TemplateIcon) => void;
}) {
  const { t } = useLingui();
  const selected = normalizeTemplateIcon(value);
  const [open, setOpen] = useState(false);
  const [tab, setTab] = useState<"icons" | "emojis">(
    selected.type === "emoji" ? "emojis" : "icons",
  );
  const [iconSearch, setIconSearch] = useState("");
  const [emojiSearch, setEmojiSearch] = useState("");
  const [customColorOpen, setCustomColorOpen] = useState(false);
  const [recentEmojiIds, setRecentEmojiIds] = useState(loadRecentEmojiIds);
  const [iconColor, setIconColor] = useState(
    selected.type === "icon" ? selected.color : DEFAULT_TEMPLATE_ICON.color,
  );
  const [lastIconValue, setLastIconValue] = useState(
    selected.type === "icon" ? selected.value : DEFAULT_TEMPLATE_ICON.value,
  );
  const emojiCategoryLabels: Record<string, string> = {
    people: t`Smileys & People`,
    nature: t`Animals & Nature`,
    foods: t`Food & Drink`,
    activity: t`Activity`,
    places: t`Travel & Places`,
    objects: t`Objects`,
    symbols: t`Symbols`,
    flags: t`Flags`,
  };

  const filteredIcons = useMemo(() => {
    const query = iconSearch.trim().toLowerCase();
    return query
      ? TEMPLATE_ICONS.filter((icon) => icon.search.includes(query))
      : TEMPLATE_ICONS;
  }, [iconSearch]);
  const filteredEmojiCategories = useMemo(() => {
    const query = emojiSearch.trim().toLowerCase();
    if (!query) {
      return EMOJI_CATEGORIES;
    }

    return EMOJI_CATEGORIES.flatMap((category) => {
      const items = category.items.filter((emoji) =>
        emoji.search.includes(query),
      );
      return items.length > 0 ? [{ ...category, items }] : [];
    });
  }, [emojiSearch]);
  const frequentEmojis = useMemo(() => {
    const ids = [...new Set([...recentEmojiIds, ...FREQUENT_EMOJI_IDS])];
    return ids.flatMap((id) => (EMOJI_ITEMS[id] ? [EMOJI_ITEMS[id]] : []));
  }, [recentEmojiIds]);

  const selectIcon = (iconValue: string) => {
    setLastIconValue(iconValue);
    onChange({ type: "icon", value: iconValue, color: iconColor });
    setOpen(false);
  };
  const selectColor = (color: string) => {
    setIconColor(color);
    onChange({ type: "icon", value: lastIconValue, color });
  };
  const selectEmoji = (emoji: EmojiItem) => {
    const nextRecent = [
      emoji.id,
      ...recentEmojiIds.filter((id) => id !== emoji.id),
    ].slice(0, 24);
    setRecentEmojiIds(nextRecent);
    try {
      window.localStorage.setItem(
        RECENT_EMOJIS_KEY,
        JSON.stringify(nextRecent),
      );
    } catch {
      // Recent emoji history is optional in restricted webviews.
    }
    onChange({ type: "emoji", value: emoji.native });
    setOpen(false);
  };

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className={cn([
            "border-border bg-muted/60 hover:bg-accent relative flex size-9 shrink-0 items-center justify-center overflow-hidden rounded-md border transition-colors",
            "after:border-t-background after:absolute after:top-0 after:right-0 after:size-0 after:border-t-[8px] after:border-l-[8px] after:border-l-transparent",
          ])}
          aria-label={t`Choose template icon`}
        >
          <TemplateIconGlyph
            icon={selected}
            className={selected.type === "emoji" ? "text-lg" : "size-[18px]"}
          />
        </button>
      </PopoverTrigger>
      <PopoverContent
        variant="app"
        align="start"
        sideOffset={6}
        className="w-[420px] max-w-[calc(100vw-24px)]"
      >
        <AppFloatingPanel className="overflow-hidden">
          <div className="border-border flex h-12 items-end gap-6 border-b px-4">
            {(["icons", "emojis"] as const).map((nextTab) => (
              <button
                key={nextTab}
                type="button"
                role="tab"
                aria-selected={tab === nextTab}
                onClick={() => setTab(nextTab)}
                className={cn([
                  "relative h-full pt-1 text-sm font-medium capitalize",
                  tab === nextTab
                    ? "text-foreground after:bg-primary after:absolute after:right-0 after:bottom-0 after:left-0 after:h-0.5"
                    : "text-muted-foreground hover:text-foreground",
                ])}
              >
                {nextTab === "icons" ? t`Icons` : t`Emojis`}
              </button>
            ))}
          </div>

          {tab === "icons" ? (
            <div>
              <div className="border-border border-b px-4 py-3">
                <div className="flex items-center justify-between">
                  {ICON_COLORS.map((color) => (
                    <button
                      key={color}
                      type="button"
                      className="relative flex size-7 items-center justify-center rounded-full"
                      style={{ backgroundColor: color }}
                      onClick={() => {
                        setCustomColorOpen(false);
                        selectColor(color);
                      }}
                      aria-label={`Use ${color}`}
                    >
                      {iconColor.toLowerCase() === color.toLowerCase() ? (
                        <Check className="size-4 text-white" />
                      ) : null}
                    </button>
                  ))}
                  <div className="bg-border h-7 w-px" />
                  <button
                    type="button"
                    className={cn([
                      "size-7 rounded-full bg-[conic-gradient(from_180deg,red,#ff0,#0f0,#0ff,#00f,#f0f,red)]",
                      customColorOpen && "ring-primary ring-2 ring-offset-2",
                    ])}
                    onClick={() => setCustomColorOpen((current) => !current)}
                    aria-label={t`Choose custom color`}
                  />
                </div>

                {customColorOpen ? (
                  <div className="mt-3">
                    <div className="mb-2 flex items-center gap-2">
                      <span
                        className="size-6 rounded-full"
                        style={{ backgroundColor: iconColor }}
                      />
                      <span className="text-muted-foreground text-xs font-medium">
                        HEX
                      </span>
                      <HexColorInput
                        color={iconColor}
                        onChange={selectColor}
                        prefixed
                        className="min-w-0 flex-1 bg-transparent text-sm uppercase outline-hidden"
                      />
                    </div>
                    <HexColorPicker
                      color={iconColor}
                      onChange={selectColor}
                      className="template-color-picker! h-36! w-full!"
                    />
                  </div>
                ) : null}
              </div>

              <SearchField
                value={iconSearch}
                onChange={setIconSearch}
                placeholder={t`Search icons...`}
              />
              <div className="scroll-fade-y max-h-[360px] overflow-y-auto p-3">
                <div className="grid grid-cols-12 gap-1">
                  {filteredIcons.map((icon) => (
                    <button
                      key={icon.value}
                      type="button"
                      onClick={() => selectIcon(icon.value)}
                      className={cn([
                        "hover:bg-accent flex size-7 items-center justify-center rounded-md transition-colors",
                        selected.type === "icon" &&
                          selected.value === icon.value &&
                          "bg-accent",
                      ])}
                      title={icon.search}
                      aria-label={icon.search}
                    >
                      <icon.component
                        className="size-[18px]"
                        style={{ color: iconColor }}
                      />
                    </button>
                  ))}
                </div>
                {filteredIcons.length === 0 ? (
                  <p className="text-muted-foreground py-8 text-center text-sm">
                    {t`No icons found`}
                  </p>
                ) : null}
              </div>
            </div>
          ) : (
            <div>
              <SearchField
                value={emojiSearch}
                onChange={setEmojiSearch}
                placeholder={t`Search emoji...`}
              />
              <div className="scroll-fade-y max-h-[480px] overflow-y-auto px-4 py-3">
                {!emojiSearch.trim() ? (
                  <EmojiSection
                    title={t`Frequently used`}
                    emojis={frequentEmojis}
                    onSelect={selectEmoji}
                  />
                ) : null}
                {filteredEmojiCategories.map((category) => (
                  <EmojiSection
                    key={category.id}
                    title={emojiCategoryLabels[category.id] ?? category.id}
                    emojis={category.items}
                    onSelect={selectEmoji}
                  />
                ))}
                {filteredEmojiCategories.length === 0 ? (
                  <p className="text-muted-foreground py-8 text-center text-sm">
                    {t`No emoji found`}
                  </p>
                ) : null}
              </div>
            </div>
          )}
        </AppFloatingPanel>
      </PopoverContent>
    </Popover>
  );
}

function EmojiSection({
  title,
  emojis,
  onSelect,
}: {
  title: string;
  emojis: EmojiItem[];
  onSelect: (emoji: EmojiItem) => void;
}) {
  return (
    <section className="mb-4 last:mb-0">
      <h3 className="text-muted-foreground mb-1.5 text-sm font-medium">
        {title}
      </h3>
      <div className="grid grid-cols-12 gap-1">
        {emojis.map((emoji) => (
          <button
            key={emoji.id}
            type="button"
            onClick={() => onSelect(emoji)}
            className="hover:bg-accent flex size-7 items-center justify-center rounded-md text-lg transition-colors"
            title={emoji.name}
            aria-label={emoji.name}
          >
            {emoji.native}
          </button>
        ))}
      </div>
    </section>
  );
}
