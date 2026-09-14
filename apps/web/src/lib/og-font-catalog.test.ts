import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import test from "node:test";
import sharp from "sharp";

import { OG_FONT_CATALOG, getOgFontFamilies } from "./og-font-catalog.ts";
import { renderSharedNoteOgImage } from "./og-image.ts";

const root = new URL("../../public/fonts/catalog/", import.meta.url);
const manifest = JSON.parse(
  readFileSync(new URL("manifest.json", root), "utf8"),
) as {
  family: string;
  file: string;
  source: string;
  metadataSource: string;
  subsets: string[];
  languages: string[];
}[];

test("catalog families ship their fonts, licenses, and declared Google Fonts coverage", () => {
  for (const family of new Set(
    OG_FONT_CATALOG.flatMap(({ serif, sans }) => [serif, sans]),
  )) {
    const font = manifest.find((entry) => entry.family === family);
    assert.ok(font, family);
    assert.ok(existsSync(new URL(font.file, root)), font.file);
    assert.ok(existsSync(new URL(`${font.file.split("/")[0]}/OFL.txt`, root)));
    assert.ok(font.languages.length > 0, family);
    assert.match(font.metadataSource, /google\/fonts\/[a-f0-9]{40}\//);
  }
});

test("every script mapping is backed by a declared Google Fonts subset", () => {
  const subsetNames: Record<string, string> = {
    Hangul: "korean",
    Hiragana: "japanese",
    Katakana: "japanese",
    Hans: "chinese-simplified",
    Hant: "chinese-traditional",
  };
  for (const { script, serif, sans } of OG_FONT_CATALOG) {
    const subset = subsetNames[script] ?? script.toLowerCase();
    for (const family of [serif, sans]) {
      assert.ok(
        manifest
          .find((entry) => entry.family === family)
          ?.subsets.includes(subset),
        `${family}: ${subset}`,
      );
    }
  }
});

test("selects fonts for every script present without letting language hints hide text", () => {
  const fonts = getOgFontFamilies("한국어 العربية हिन्दी Ελληνικά", ["en"]);
  for (const family of [
    "Noto Serif KR",
    "Noto Naskh Arabic",
    "Noto Serif Devanagari",
    "Noto Serif",
  ]) {
    assert.ok(fonts.serif.includes(`'${family}'`), family);
  }
  assert.ok(fonts.sans.includes("'Noto Sans Arabic'"));
  assert.ok(fonts.serif.startsWith("'Redaction'"));
  assert.ok(fonts.sans.startsWith("'SF Pro Text'"));
  assert.equal(
    getOgFontFamilies("Hello", ["ko"]).serif,
    "'Redaction', 'Noto Serif', serif",
  );
});

test("resolves shared Han glyphs using content and optional language hints", () => {
  assert.match(getOgFontFamilies("日本語の会議").serif, /Noto Serif JP/);
  assert.match(getOgFontFamilies("한국 漢字").serif, /Noto Serif KR/);
  assert.match(getOgFontFamilies("中文", ["zh-TW"]).serif, /Noto Serif TC/);
  assert.match(getOgFontFamilies("中文", ["zh-CN"]).serif, /Noto Serif SC/);
  assert.match(getOgFontFamilies("中文", ["ja"]).serif, /Noto Serif JP/);
  assert.match(getOgFontFamilies("會議記錄").serif, /Noto Serif TC/);
  assert.match(getOgFontFamilies("会议记录").serif, /Noto Serif SC/);
  assert.match(
    getOgFontFamilies("中文", ["invalid_locale"]).serif,
    /Noto Serif SC/,
  );
});

const samples = [
  "Réunion tiếng Việt",
  "Ελληνικά",
  "Українська",
  "한국어",
  "日本語の会議",
  "简体中文",
  "繁體中文",
  "العربية",
  "עברית",
  "हिन्दी",
  "বাংলা",
  "ગુજરાતી",
  "ਪੰਜਾਬੀ",
  "தமிழ்",
  "తెలుగు",
  "ಕನ್ನಡ",
  "മലയാളം",
  "සිංහල",
  "ภาษาไทย",
  "ພາສາລາວ",
  "ភាសាខ្មែរ",
  "မြန်မာ",
  "ქართული",
  "Հայերեն",
  "አማርኛ",
  "བོད་ཡིག",
  "ଓଡ଼ିଆ",
];
for (const title of samples) {
  test(`rasterizes catalog text: ${title}`, async () => {
    const response = await renderSharedNoteOgImage({
      title,
      summary: title,
      participants: [title],
    });
    const png = Buffer.from(await response.arrayBuffer());
    const metadata = await sharp(png).metadata();
    assert.equal(metadata.width, 1200);
    assert.equal(metadata.height, 630);
    assert.ok(png.length > 5000);
  });
}
