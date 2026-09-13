// Declared subsets, languages, licenses, and pinned sources live in public/fonts/catalog/manifest.json.
export const OG_FONT_CATALOG = [
  { script: "Latin", serif: "Noto Serif", sans: "Noto Sans" },
  { script: "Greek", serif: "Noto Serif", sans: "Noto Sans" },
  { script: "Cyrillic", serif: "Noto Serif", sans: "Noto Sans" },
  { script: "Hangul", serif: "Noto Serif KR", sans: "Noto Sans KR" },
  { script: "Hiragana", serif: "Noto Serif JP", sans: "Noto Sans JP" },
  { script: "Katakana", serif: "Noto Serif JP", sans: "Noto Sans JP" },
  { script: "Arabic", serif: "Noto Naskh Arabic", sans: "Noto Sans Arabic" },
  { script: "Hebrew", serif: "Noto Serif Hebrew", sans: "Noto Sans Hebrew" },
  {
    script: "Devanagari",
    serif: "Noto Serif Devanagari",
    sans: "Noto Sans Devanagari",
  },
  { script: "Bengali", serif: "Noto Serif Bengali", sans: "Noto Sans Bengali" },
  {
    script: "Gujarati",
    serif: "Noto Serif Gujarati",
    sans: "Noto Sans Gujarati",
  },
  {
    script: "Gurmukhi",
    serif: "Noto Serif Gurmukhi",
    sans: "Noto Sans Gurmukhi",
  },
  { script: "Tamil", serif: "Noto Serif Tamil", sans: "Noto Sans Tamil" },
  { script: "Telugu", serif: "Noto Serif Telugu", sans: "Noto Sans Telugu" },
  { script: "Kannada", serif: "Noto Serif Kannada", sans: "Noto Sans Kannada" },
  {
    script: "Malayalam",
    serif: "Noto Serif Malayalam",
    sans: "Noto Sans Malayalam",
  },
  { script: "Sinhala", serif: "Noto Serif Sinhala", sans: "Noto Sans Sinhala" },
  { script: "Thai", serif: "Noto Serif Thai", sans: "Noto Sans Thai" },
  { script: "Lao", serif: "Noto Serif Lao", sans: "Noto Sans Lao" },
  { script: "Khmer", serif: "Noto Serif Khmer", sans: "Noto Sans Khmer" },
  { script: "Myanmar", serif: "Noto Serif Myanmar", sans: "Noto Sans Myanmar" },
  {
    script: "Georgian",
    serif: "Noto Serif Georgian",
    sans: "Noto Sans Georgian",
  },
  {
    script: "Armenian",
    serif: "Noto Serif Armenian",
    sans: "Noto Sans Armenian",
  },
  {
    script: "Ethiopic",
    serif: "Noto Serif Ethiopic",
    sans: "Noto Sans Ethiopic",
  },
  {
    script: "Tibetan",
    serif: "Noto Serif Tibetan",
    sans: "Noto Serif Tibetan",
  },
  { script: "Oriya", serif: "Noto Serif Oriya", sans: "Noto Sans Oriya" },
  { script: "Hans", serif: "Noto Serif SC", sans: "Noto Sans SC" },
  { script: "Hant", serif: "Noto Serif TC", sans: "Noto Sans TC" },
] as const;

const scriptPatterns = OG_FONT_CATALOG.filter(
  ({ script }) => script !== "Hans" && script !== "Hant",
).map((entry) => ({
  ...entry,
  pattern: new RegExp(`\\p{Script_Extensions=${entry.script}}`, "u"),
}));

const traditionalHanMarkers =
  /[繁體臺灣萬與專業為會議記錄聽說學習國門開關後來這裡點線網頁]/u;
const simplifiedHanMarkers =
  /[简体台湾万与专业为会议记录听说学习国门开关后来这里点线网页]/u;

export function getOgFontFamilies(text: string, languageHints: string[] = []) {
  const matches = scriptPatterns.filter(({ pattern }) => pattern.test(text));
  const families: { serif: string; sans: string }[] = [...matches];

  if (/\p{Script=Han}/u.test(text)) {
    const script = getHanScript(text, languageHints);
    families.unshift(OG_FONT_CATALOG.find((entry) => entry.script === script)!);
  }

  const stack = (brand: string, kind: "serif" | "sans", generic: string) =>
    [
      ...new Set([
        brand,
        ...families.map((family) => family[kind]),
        kind === "serif" ? "Noto Serif" : "Noto Sans",
      ]),
    ]
      .map((family) => `'${family}'`)
      .concat(generic)
      .join(", ");
  return {
    serif: stack("Redaction", "serif", "serif"),
    sans: stack("SF Pro Text", "sans", "sans-serif"),
  };
}

function getHanScript(text: string, languageHints: string[]) {
  for (const language of languageHints) {
    let locale: Intl.Locale;
    try {
      locale = new Intl.Locale(language).maximize();
    } catch {
      continue;
    }
    if (locale.language === "ja") return "Hiragana";
    if (locale.language === "ko") return "Hangul";
    if (locale.language === "zh" || locale.language === "yue") {
      return locale.script === "Hant" ? "Hant" : "Hans";
    }
  }
  if (/[\p{Script=Hiragana}\p{Script=Katakana}]/u.test(text)) return "Hiragana";
  if (/\p{Script=Hangul}/u.test(text)) return "Hangul";
  const usesTraditional = traditionalHanMarkers.test(text);
  const usesSimplified = simplifiedHanMarkers.test(text);
  if (usesTraditional !== usesSimplified) {
    return usesTraditional ? "Hant" : "Hans";
  }
  return "Hans";
}
