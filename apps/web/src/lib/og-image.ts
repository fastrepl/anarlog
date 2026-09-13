import "./og-fonts.ts";

import sharp from "sharp";

import {
  AVATAR_RASTER_SIZE,
  avatarInitials,
  createAvatarGradient,
  createAvatarPixels,
  type AvatarRecipe,
} from "@anlg/ui/lib/avatar";

import { ANARLOG_WORDMARK } from "./brand-assets.ts";
import { getOgFontFamilies } from "./og-font-catalog.ts";
import { createSharedNoteParticipantPresentation } from "./shared-note-presentation.ts";

const OG_WIDTH = 1200;
const OG_HEIGHT = 630;
const CONTENT_INSET_X = 72;
const CONTENT_RIGHT_X = OG_WIDTH - CONTENT_INSET_X;
const AVATAR_RADIUS = 28;
const AVATAR_STEP = 42;
const AVATAR_LABEL_GAP = 16;
const SUMMARY_FONT_SIZE = 31;
const SUMMARY_LINE_HEIGHT = 42;
const SUMMARY_MAX_LINES = 2;
const WORDMARK_WIDTH = 165;
const ROOT_OG_BACKGROUND = "#ffe09d";
const CACHE_CONTROL =
  "public, max-age=0, s-maxage=86400, stale-while-revalidate=604800";
const SHARED_NOTE_CACHE_CONTROL = "public, max-age=0, s-maxage=60";

type BlogOgImageInput = {
  languageHints?: string[];
  title: string;
  description?: string;
  date?: string;
  author?: string;
};

type SharedNoteOgImageInput = {
  languageHints?: string[];
  title: string;
  summary?: string;
  participants?: string[];
  meetingAt?: string;
};

function clampText(value: string | undefined, maxLength: number) {
  if (!value) return "";
  const normalized = value.replace(/\s+/g, " ").trim();
  if (normalized.length <= maxLength) return normalized;
  return `${normalized.slice(0, maxLength - 1).trimEnd()}...`;
}

function escapeXml(value: string) {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

const graphemeSegmenter = new Intl.Segmenter(undefined, {
  granularity: "grapheme",
});

function splitGraphemes(value: string) {
  return [...graphemeSegmenter.segment(value)].map(({ segment }) => segment);
}

function wrapText(
  value: string,
  fontSize: number,
  maxWidth: number,
  maxLines: number,
) {
  const words = value.split(/\s+/).filter(Boolean);
  const pieces = words.flatMap((word) => {
    const chunks: string[] = [];
    let current = "";
    for (const grapheme of splitGraphemes(word)) {
      if (
        current &&
        estimateTextWidth(`${current}${grapheme}`, fontSize) > maxWidth
      ) {
        chunks.push(current);
        current = "";
      }
      current += grapheme;
    }
    if (current) chunks.push(current);
    return chunks.map((text, index) => ({
      text,
      prependSpace: index === 0,
    }));
  });
  const lines: string[] = [];
  let current = "";
  let truncated = false;

  for (const piece of pieces) {
    const next = `${current}${current && piece.prependSpace ? " " : ""}${piece.text}`;
    if (estimateTextWidth(next, fontSize) <= maxWidth) {
      current = next;
      continue;
    }

    if (current) lines.push(current);
    if (lines.length === maxLines) {
      truncated = true;
      break;
    }
    current = piece.text;
  }

  if (!truncated && current && lines.length < maxLines) {
    lines.push(current);
  }

  if (truncated) {
    lines[lines.length - 1] = ellipsizeToWidth(
      lines[lines.length - 1] ?? "",
      fontSize,
      maxWidth,
    );
  }

  return lines;
}

function wrapSansText(
  value: string,
  fontSize: number,
  maxWidth: number,
  maxLines: number,
) {
  if (!value) return [];

  const words = value.split(/\s+/).filter(Boolean);
  const lines: string[] = [];
  let current = "";
  const fits = (text: string) => estimateTextWidth(text, fontSize) <= maxWidth;

  for (const word of words) {
    const next = current ? `${current} ${word}` : word;
    if (fits(next)) {
      current = next;
      continue;
    }

    if (current) {
      lines.push(current);
      current = word;
    } else {
      lines.push(ellipsizeToWidth(word, fontSize, maxWidth));
      current = "";
    }

    if (lines.length === maxLines) {
      current = "";
      break;
    }
  }

  if (current && lines.length < maxLines) {
    lines.push(current);
  }

  const consumed = lines.join(" ");
  if (lines.length > 0 && words.join(" ").length > consumed.length) {
    lines[lines.length - 1] = ellipsizeToWidth(
      lines[lines.length - 1] ?? "",
      fontSize,
      maxWidth,
    );
  }

  return lines.filter(Boolean);
}

function ellipsizeToWidth(value: string, fontSize: number, maxWidth: number) {
  const fits = (text: string) => estimateTextWidth(text, fontSize) <= maxWidth;
  let next = value.replace(/\.+$/, "").trimEnd();
  if (fits(`${next}...`)) return `${next}...`;

  const parts = next.split(/\s+/).filter(Boolean);
  while (parts.length > 1) {
    parts.pop();
    next = parts.join(" ");
    if (fits(`${next}...`)) return `${next}...`;
  }

  const graphemes = splitGraphemes(parts[0] ?? "");
  while (graphemes.length > 1 && !fits(`${graphemes.join("")}...`)) {
    graphemes.pop();
  }
  next = graphemes.join("");
  return next ? `${next}...` : "";
}

function formatDate(date: string | undefined) {
  if (!date) return "";
  const parsed = new Date(date);
  if (Number.isNaN(parsed.getTime())) return date;
  return parsed.toLocaleDateString("en-US", {
    month: "long",
    day: "numeric",
    timeZone: "UTC",
    year: "numeric",
  });
}

function createAnarlogWordmark({
  x,
  y,
  width,
}: {
  x: number;
  y: number;
  width: number;
}) {
  const height = (width * ANARLOG_WORDMARK.height) / ANARLOG_WORDMARK.width;

  return `<svg data-wordmark="anarlog" aria-label="Anarlog" x="${x}" y="${y}" width="${width}" height="${height.toFixed(1)}" viewBox="0 0 ${ANARLOG_WORDMARK.width} ${ANARLOG_WORDMARK.height}" preserveAspectRatio="xMidYMid meet"><path d="${ANARLOG_WORDMARK.path}" fill="#000000"/></svg>`;
}

function participantAvatarRecipe(seed: string): AvatarRecipe {
  return {
    seed,
    colorCount: 4,
    sphereCount: 4,
    dither: 0.3,
    renderStyle: "dithered",
  };
}

function createAvatarGradientSvg(seed: string, id: string) {
  const { angle, colors } = createAvatarGradient(seed);
  const radians = ((angle - 90) * Math.PI) / 180;
  const offsetX = Math.cos(radians) * 50;
  const offsetY = Math.sin(radians) * 50;

  return `<linearGradient id="${id}" x1="${50 - offsetX}%" y1="${50 - offsetY}%" x2="${50 + offsetX}%" y2="${50 + offsetY}%">${colors
    .map(
      ([red, green, blue], index) =>
        `<stop offset="${(index / (colors.length - 1)) * 100}%" stop-color="rgb(${Math.round(red)} ${Math.round(green)} ${Math.round(blue)})"/>`,
    )
    .join("")}</linearGradient>`;
}

function createParticipantAvatarStack(
  participants: string[],
  avatarImages: string[],
  centerY: number,
  fontFamily: string,
) {
  const avatars = participants.map((participant, index) => ({
    image: avatarImages[index],
    label: avatarInitials(participant),
    seed: participant,
  }));

  return avatars
    .map((avatar, index) => {
      const centerX = CONTENT_INSET_X + AVATAR_RADIUS + index * AVATAR_STEP;
      const gradientId = `avatar-gradient-${index}`;
      const clipId = `avatar-clip-${index}`;
      return `<defs>${createAvatarGradientSvg(avatar.seed, gradientId)}<clipPath id="${clipId}"><circle cx="${centerX}" cy="${centerY}" r="${AVATAR_RADIUS}"/></clipPath></defs><g data-avatar="participant" data-avatar-renderer="app"><circle cx="${centerX}" cy="${centerY}" r="${AVATAR_RADIUS}" fill="url(#${gradientId})"/>${avatar.image ? `<image href="${avatar.image}" x="${centerX - AVATAR_RADIUS}" y="${centerY - AVATAR_RADIUS}" width="${AVATAR_RADIUS * 2}" height="${AVATAR_RADIUS * 2}" preserveAspectRatio="xMidYMid slice" clip-path="url(#${clipId})"/>` : ""}<circle cx="${centerX}" cy="${centerY}" r="${AVATAR_RADIUS + 2}" fill="none" stroke="${ROOT_OG_BACKGROUND}" stroke-width="4"/><text x="${centerX}" y="${centerY + 7}" fill="#ffffff" fill-opacity="0.82" font-family="${fontFamily}" font-size="18" font-weight="700" text-anchor="middle" style="mix-blend-mode:overlay">${escapeXml(avatar.label)}</text></g>`;
    })
    .reverse()
    .join("");
}

function estimateTextWidth(value: string, fontSize: number) {
  return splitGraphemes(value).reduce((width, grapheme) => {
    if (/\s/u.test(grapheme)) return width + fontSize * 0.28;
    if (
      /[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}\p{Script=Hangul}\p{Extended_Pictographic}]/u.test(
        grapheme,
      )
    )
      return width + fontSize;
    if (/[ilI.,'`]/u.test(grapheme)) return width + fontSize * 0.3;
    if (/[MW@%]/u.test(grapheme)) return width + fontSize * 0.82;
    return width + fontSize * 0.56;
  }, 0);
}

async function createParticipantAvatarImages(participants: string[]) {
  return Promise.all(
    participants.map(async (participant) => {
      const pixels = createAvatarPixels(participantAvatarRecipe(participant));
      const png = await sharp(Buffer.from(pixels), {
        raw: {
          width: AVATAR_RASTER_SIZE,
          height: AVATAR_RASTER_SIZE,
          channels: 4,
        },
      })
        .png()
        .toBuffer();
      return `data:image/png;base64,${png.toString("base64")}`;
    }),
  );
}

export function createBlogOgSvg(input: BlogOgImageInput) {
  const fonts = getOgFontFamilies(
    [input.title, input.description, input.author].filter(Boolean).join(" "),
    input.languageHints,
  );
  const title = wrapText(clampText(input.title, 96), 76, 1028, 3);
  const description = wrapText(clampText(input.description, 150), 32, 1024, 2);
  const meta = [input.author, formatDate(input.date)]
    .filter(Boolean)
    .join(" - ");
  const titleStartY = title.length === 1 ? 246 : title.length === 2 ? 207 : 171;
  const descriptionStartY = titleStartY + title.length * 86 + 36;

  return `<?xml version="1.0" encoding="UTF-8"?>
<svg width="${OG_WIDTH}" height="${OG_HEIGHT}" viewBox="0 0 ${OG_WIDTH} ${OG_HEIGHT}" fill="none" xmlns="http://www.w3.org/2000/svg">
  <rect width="${OG_WIDTH}" height="${OG_HEIGHT}" fill="#ffffff"/>
  <path d="M86 504 H1114" stroke="#d8d1c8" stroke-width="2"/>
  <g opacity="0.22">
    ${Array.from({ length: 18 }, (_, index) => {
      const x = 92 + index * 60;
      return `<path d="M${x} 86 V544" stroke="#c5bbb0" stroke-width="1"/>`;
    }).join("")}
    ${Array.from({ length: 8 }, (_, index) => {
      const y = 94 + index * 56;
      return `<path d="M86 ${y} H1114" stroke="#c5bbb0" stroke-width="1"/>`;
    }).join("")}
  </g>
  ${title
    .map(
      (line, index) =>
        `<text x="86" y="${titleStartY + index * 86}" fill="#181613" font-family="${fonts.serif}" font-size="76" font-weight="400">${escapeXml(line)}</text>`,
    )
    .join("")}
  ${description
    .map(
      (line, index) =>
        `<text x="90" y="${descriptionStartY + index * 42}" fill="#57534e" font-family="${fonts.sans}" font-size="32" font-weight="500">${escapeXml(line)}</text>`,
    )
    .join("")}
  <text x="86" y="552" fill="#756b5d" font-family="${fonts.sans}" font-size="26" font-weight="600">${escapeXml(meta || "anarlog")}</text>
  ${createAnarlogWordmark({ x: 962, y: 516, width: 152 })}
</svg>`;
}

export function createSharedNoteOgSvg(
  input: SharedNoteOgImageInput,
  avatarImages: string[] = [],
) {
  const fonts = getOgFontFamilies(
    [input.title, input.summary, ...(input.participants ?? [])]
      .filter(Boolean)
      .join(" "),
    input.languageHints,
  );
  const normalizedTitle = clampText(input.title, 120) || "Shared note";
  const titleFontSize = normalizedTitle.length > 72 ? 64 : 76;
  const title = wrapText(normalizedTitle, titleFontSize, 1056, 3);
  const participantPresentation = createSharedNoteParticipantPresentation(
    input.participants ?? [],
  );
  const participantSummary = clampText(participantPresentation.label, 42);
  const avatarParticipants = participantPresentation.avatarParticipants;
  const summary = wrapSansText(
    clampText(input.summary, 180),
    SUMMARY_FONT_SIZE,
    CONTENT_RIGHT_X - CONTENT_INSET_X,
    SUMMARY_MAX_LINES,
  );
  const titleStartY = title.length === 1 ? 152 : title.length === 2 ? 116 : 90;
  const titleEndY = titleStartY + (title.length - 1) * 82;
  const summaryY = titleEndY + 58;
  const date = formatDate(input.meetingAt) || "Date unavailable";
  const footerCenterY = 500;
  const avatarCount = avatarParticipants.length;
  const firstAvatarX = CONTENT_INSET_X + AVATAR_RADIUS;
  const participantX = avatarCount
    ? firstAvatarX +
      (avatarCount - 1) * AVATAR_STEP +
      AVATAR_RADIUS +
      AVATAR_LABEL_GAP
    : CONTENT_INSET_X;
  const estimatedParticipantTextWidth = estimateTextWidth(
    participantSummary,
    27,
  );
  const participantTextWidth = Math.min(estimatedParticipantTextWidth, 440);
  const participantTextLength =
    estimatedParticipantTextWidth > participantTextWidth
      ? ` textLength="${participantTextWidth}" lengthAdjust="spacingAndGlyphs"`
      : "";
  const separatorX = participantX + participantTextWidth + 22;
  const dateX = separatorX + 20;

  return `<?xml version="1.0" encoding="UTF-8"?>
<svg width="${OG_WIDTH}" height="${OG_HEIGHT}" viewBox="0 0 ${OG_WIDTH} ${OG_HEIGHT}" fill="none" xmlns="http://www.w3.org/2000/svg">
  <rect width="${OG_WIDTH}" height="${OG_HEIGHT}" fill="${ROOT_OG_BACKGROUND}"/>
  <path d="M${CONTENT_INSET_X} 424 H${CONTENT_RIGHT_X}" stroke="#cfc6ba" stroke-width="2"/>
  ${title
    .map(
      (line, index) =>
        `<text x="${CONTENT_INSET_X}" y="${titleStartY + index * 82}" fill="#181613" font-family="${fonts.serif}" font-size="${titleFontSize}" font-weight="400">${escapeXml(line)}</text>`,
    )
    .join("")}
  ${summary
    .map(
      (line, index) =>
        `<text data-summary="meeting" x="${CONTENT_INSET_X}" y="${summaryY + index * SUMMARY_LINE_HEIGHT}" fill="#57534e" font-family="${fonts.sans}" font-size="${SUMMARY_FONT_SIZE}" font-weight="500">${escapeXml(line)}</text>`,
    )
    .join("")}
  ${createParticipantAvatarStack(avatarParticipants, avatarImages, footerCenterY, fonts.sans)}
  <text x="${participantX}" y="${footerCenterY + 9}"${participantTextLength} fill="#37322d" font-family="${fonts.sans}" font-size="27" font-weight="600">${escapeXml(participantSummary)}</text>
  <circle cx="${separatorX}" cy="${footerCenterY}" r="3" fill="#9d9387"/>
  <text x="${dateX}" y="${footerCenterY + 9}" fill="#57534e" font-family="${fonts.sans}" font-size="27" font-weight="500">${escapeXml(date)}</text>
  ${createAnarlogWordmark({
    x: CONTENT_RIGHT_X - WORDMARK_WIDTH,
    y: 477,
    width: WORDMARK_WIDTH,
  })}
</svg>`;
}

async function renderOgImage(svg: string, cacheControl: string) {
  const png = await sharp(Buffer.from(svg)).png().toBuffer();

  return new Response(new Uint8Array(png), {
    headers: {
      "Cache-Control": cacheControl,
      "Content-Type": "image/png",
    },
  });
}

export async function renderBlogOgImage(input: BlogOgImageInput) {
  return renderOgImage(createBlogOgSvg(input), CACHE_CONTROL);
}

export async function renderSharedNoteOgImage(input: SharedNoteOgImageInput) {
  const participantPresentation = createSharedNoteParticipantPresentation(
    input.participants ?? [],
  );
  const avatarImages = await createParticipantAvatarImages(
    participantPresentation.avatarParticipants,
  );
  return renderOgImage(
    createSharedNoteOgSvg(input, avatarImages),
    SHARED_NOTE_CACHE_CONTROL,
  );
}
