import emojiData, { type Emoji, type EmojiMartData } from "@emoji-mart/data";
import { useLingui } from "@lingui/react/macro";
import { Check, MagnifyingGlass, X } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMemo, useState } from "react";
import { HexColorInput, HexColorPicker } from "react-colorful";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@anlg/ui/components/ui/popover";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

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
  const { t } = useLingui();
  return (
    <div {...stylex.props(styles.searchField)}>
      <MagnifyingGlass {...stylex.props(styles.searchIcon)} />
      <input
        type="text"
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
        {...stylex.props(styles.searchInput)}
      />
      {value ? (
        <button
          type="button"
          onClick={() => onChange("")}
          {...stylex.props(styles.clearButton)}
          aria-label={t`Clear search`}
        >
          <X {...stylex.props(styles.clearIcon)} />
        </button>
      ) : null}
    </div>
  );
}

export function TemplateIconPicker({
  value,
  onChange,
  size = "default",
}: {
  value: TemplateIcon;
  onChange: (value: TemplateIcon) => void;
  size?: "default" | "sm";
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
          {...stylex.props([
            styles.trigger,
            size === "sm" ? styles.smallTrigger : styles.defaultTrigger,
          ])}
          aria-label={t`Choose template icon`}
        >
          <TemplateIconGlyph
            icon={selected}
            sx={
              selected.type === "emoji"
                ? size === "sm"
                  ? styles.smallEmoji
                  : styles.defaultEmoji
                : size === "sm"
                  ? styles.smallIcon
                  : styles.defaultIcon
            }
          />
        </button>
      </PopoverTrigger>
      <PopoverContent
        variant="app"
        align="start"
        sideOffset={6}
        sx={styles.popover}
      >
        <AppFloatingPanel sx={styles.panel}>
          <div {...stylex.props(styles.tabs)}>
            {(["icons", "emojis"] as const).map((nextTab) => (
              <button
                key={nextTab}
                type="button"
                role="tab"
                aria-selected={tab === nextTab}
                onClick={() => setTab(nextTab)}
                {...stylex.props([
                  styles.tab,
                  tab === nextTab ? styles.activeTab : styles.inactiveTab,
                ])}
              >
                {nextTab === "icons" ? t`Icons` : t`Emojis`}
              </button>
            ))}
          </div>

          {tab === "icons" ? (
            <div>
              <div {...stylex.props(styles.colorSection)}>
                <div {...stylex.props(styles.colorRow)}>
                  {ICON_COLORS.map((color) => (
                    <button
                      key={color}
                      type="button"
                      {...stylex.props([
                        styles.colorButton,
                        styles.dynamicBackground(color),
                      ])}
                      onClick={() => {
                        setCustomColorOpen(false);
                        selectColor(color);
                      }}
                      aria-label={`Use ${color}`}
                    >
                      {iconColor.toLowerCase() === color.toLowerCase() ? (
                        <Check {...stylex.props(styles.checkIcon)} />
                      ) : null}
                    </button>
                  ))}
                  <div {...stylex.props(styles.colorDivider)} />
                  <button
                    type="button"
                    {...stylex.props([
                      styles.customColorButton,
                      customColorOpen && styles.openCustomColorButton,
                    ])}
                    onClick={() => setCustomColorOpen((current) => !current)}
                    aria-label={t`Choose custom color`}
                  />
                </div>

                {customColorOpen ? (
                  <div {...stylex.props(styles.customColor)}>
                    <div {...stylex.props(styles.customColorHeader)}>
                      <span
                        {...stylex.props([
                          styles.colorPreview,
                          styles.dynamicBackground(iconColor),
                        ])}
                      />
                      <span {...stylex.props(styles.hexLabel)}>HEX</span>
                      <HexColorInput
                        color={iconColor}
                        onChange={selectColor}
                        prefixed
                        {...stylex.props(styles.hexInput)}
                      />
                    </div>
                    <HexColorPicker
                      color={iconColor}
                      onChange={selectColor}
                      {...stylex.props(styles.colorPicker)}
                    />
                  </div>
                ) : null}
              </div>

              <SearchField
                value={iconSearch}
                onChange={setIconSearch}
                placeholder={t`Search icons...`}
              />
              <div {...mergeStyleXProps(styles.iconScroll, "scroll-fade-y")}>
                <div {...stylex.props(styles.grid)}>
                  {filteredIcons.map((icon) => (
                    <button
                      key={icon.value}
                      type="button"
                      onClick={() => selectIcon(icon.value)}
                      {...stylex.props([
                        styles.gridButton,
                        selected.type === "icon" &&
                          selected.value === icon.value &&
                          styles.selectedGridButton,
                      ])}
                      title={icon.search}
                      aria-label={icon.search}
                    >
                      <icon.component
                        {...stylex.props([
                          styles.defaultIcon,
                          styles.dynamicColor(iconColor),
                        ])}
                      />
                    </button>
                  ))}
                </div>
                {filteredIcons.length === 0 ? (
                  <p {...stylex.props(styles.emptyState)}>
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
              <div {...mergeStyleXProps(styles.emojiScroll, "scroll-fade-y")}>
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
                  <p {...stylex.props(styles.emptyState)}>
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
    <section {...stylex.props(styles.emojiSection)}>
      <h3 {...stylex.props(styles.emojiHeading)}>{title}</h3>
      <div {...stylex.props(styles.grid)}>
        {emojis.map((emoji) => (
          <button
            key={emoji.id}
            type="button"
            onClick={() => onSelect(emoji)}
            {...stylex.props([styles.gridButton, styles.emojiButton])}
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

const styles = stylex.create({
  activeTab: {
    "::after": {
      backgroundColor: colors.primary,
      bottom: 0,
      content: '""',
      height: "0.125rem",
      left: 0,
      position: "absolute",
      right: 0,
    },
    color: colors.foreground,
  },
  checkIcon: {
    color: "white",
    height: "1rem",
    width: "1rem",
  },
  clearButton: {
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: radii.sm,
    padding: "0.25rem",
  },
  clearIcon: {
    color: colors.mutedForeground,
    height: "0.875rem",
    width: "0.875rem",
  },
  colorButton: {
    alignItems: "center",
    borderRadius: radii.full,
    display: "flex",
    height: "1.75rem",
    justifyContent: "center",
    position: "relative",
    width: "1.75rem",
  },
  colorDivider: {
    backgroundColor: colors.border,
    height: "1.75rem",
    width: "1px",
  },
  colorPicker: {
    height: "9rem",
    width: "100%",
  },
  colorPreview: {
    borderRadius: radii.full,
    height: "1.5rem",
    width: "1.5rem",
  },
  colorRow: {
    alignItems: "center",
    display: "flex",
    justifyContent: "space-between",
  },
  colorSection: {
    borderBottomColor: colors.border,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    paddingBlock: "0.75rem",
    paddingInline: "1rem",
  },
  customColor: {
    marginTop: "0.75rem",
  },
  customColorButton: {
    backgroundImage:
      "conic-gradient(from 180deg, red, #ff0, #0f0, #0ff, #00f, #f0f, red)",
    borderRadius: radii.full,
    height: "1.75rem",
    width: "1.75rem",
  },
  customColorHeader: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    marginBottom: "0.5rem",
  },
  defaultEmoji: {
    fontSize: "1.125rem",
    lineHeight: "1.75rem",
  },
  defaultIcon: {
    height: "1.125rem",
    width: "1.125rem",
  },
  defaultTrigger: {
    "::after": {
      borderLeftColor: "transparent",
      borderLeftStyle: "solid",
      borderLeftWidth: "8px",
      borderTopColor: colors.background,
      borderTopStyle: "solid",
      borderTopWidth: "8px",
      content: '""',
      height: 0,
      position: "absolute",
      right: 0,
      top: 0,
      width: 0,
    },
    backgroundColor: {
      default: `color-mix(in srgb, ${colors.muted} 60%, transparent)`,
      ":hover": colors.accent,
    },
    borderColor: colors.border,
    borderStyle: "solid",
    borderWidth: "1px",
    height: "2.25rem",
    width: "2.25rem",
  },
  dynamicBackground: (color: string) => ({
    backgroundColor: color,
  }),
  dynamicColor: (color: string) => ({
    color,
  }),
  emojiButton: {
    fontSize: "1.125rem",
    lineHeight: "1.75rem",
  },
  emojiHeading: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
    marginBottom: "0.375rem",
  },
  emojiScroll: {
    maxHeight: "480px",
    overflowY: "auto",
    paddingBlock: "0.75rem",
    paddingInline: "1rem",
  },
  emojiSection: {
    marginBottom: {
      default: "1rem",
      ":last-child": 0,
    },
  },
  emptyState: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    paddingBlock: "2rem",
    textAlign: "center",
  },
  grid: {
    display: "grid",
    gap: "0.25rem",
    gridTemplateColumns: "repeat(12, minmax(0, 1fr))",
  },
  gridButton: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: radii.md,
    display: "flex",
    height: "1.75rem",
    justifyContent: "center",
    transitionDuration: "150ms",
    transitionProperty: "background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "1.75rem",
  },
  hexInput: {
    backgroundColor: "transparent",
    flex: "1",
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    minWidth: 0,
    outlineColor: "transparent",
    outlineOffset: "2px",
    outlineStyle: "solid",
    outlineWidth: "2px",
    textTransform: "uppercase",
  },
  hexLabel: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    fontWeight: 500,
    lineHeight: "1rem",
  },
  iconScroll: {
    maxHeight: "360px",
    overflowY: "auto",
    padding: "0.75rem",
  },
  inactiveTab: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
  },
  openCustomColorButton: {
    boxShadow: `0 0 0 2px ${colors.background}, 0 0 0 4px ${colors.primary}`,
  },
  panel: {
    overflow: "hidden",
  },
  popover: {
    maxWidth: "calc(100vw - 24px)",
    width: "420px",
  },
  searchField: {
    alignItems: "center",
    borderBottomColor: colors.border,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    display: "flex",
    gap: "0.5rem",
    height: "3rem",
    paddingInline: "1rem",
  },
  searchIcon: {
    color: colors.mutedForeground,
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
  searchInput: {
    "::placeholder": {
      color: colors.mutedForeground,
    },
    backgroundColor: "transparent",
    flex: "1",
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    minWidth: 0,
    outlineColor: "transparent",
    outlineOffset: "2px",
    outlineStyle: "solid",
    outlineWidth: "2px",
  },
  selectedGridButton: {
    backgroundColor: colors.accent,
  },
  smallEmoji: {
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
  },
  smallIcon: {
    height: "1rem",
    width: "1rem",
  },
  smallTrigger: {
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    height: "1.75rem",
    width: "1.75rem",
  },
  tab: {
    fontSize: "0.875rem",
    fontWeight: 500,
    height: "100%",
    lineHeight: "1.25rem",
    paddingTop: "0.25rem",
    position: "relative",
    textTransform: "capitalize",
  },
  tabs: {
    alignItems: "flex-end",
    borderBottomColor: colors.border,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    display: "flex",
    gap: "1.5rem",
    height: "3rem",
    paddingInline: "1rem",
  },
  trigger: {
    alignItems: "center",
    borderRadius: radii.md,
    display: "flex",
    flexShrink: 0,
    justifyContent: "center",
    overflow: "hidden",
    position: "relative",
    transitionDuration: "150ms",
    transitionProperty: "background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
});
