import sharp from "sharp";

import {
  AVATAR_RASTER_SIZE,
  avatarInitials,
  createAvatarGradient,
  createAvatarPixels,
  type AvatarRecipe,
} from "@anlg/ui/lib/avatar";

import { ANARLOG_WORDMARK } from "./brand-assets.ts";
import { createSharedNoteParticipantPresentation } from "./shared-note-presentation.ts";

const OG_WIDTH = 1200;
const OG_HEIGHT = 630;
const CACHE_CONTROL =
  "public, max-age=0, s-maxage=86400, stale-while-revalidate=604800";
const SHARED_NOTE_CACHE_CONTROL = "public, max-age=0, s-maxage=60";

type BlogOgImageInput = {
  title: string;
  description?: string;
  date?: string;
  author?: string;
};

type SharedNoteOgImageInput = {
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

function wrapText(value: string, maxChars: number, maxLines: number) {
  const words = value.split(/\s+/).filter(Boolean);
  const lines: string[] = [];
  let current = "";

  for (const word of words) {
    const next = current ? `${current} ${word}` : word;
    if (next.length <= maxChars) {
      current = next;
      continue;
    }

    if (current) lines.push(current);
    current = word;

    if (lines.length === maxLines) break;
  }

  if (current && lines.length < maxLines) {
    lines.push(current);
  }

  if (
    lines.length === maxLines &&
    words.join(" ").length > lines.join(" ").length
  ) {
    lines[lines.length - 1] =
      `${lines[lines.length - 1].replace(/\.+$/, "")}...`;
  }

  return lines;
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
) {
  const avatars = participants.map((participant, index) => ({
    image: avatarImages[index],
    label: avatarInitials(participant),
    seed: participant,
  }));

  return avatars
    .map((avatar, index) => {
      const centerX = 100 + index * 42;
      const gradientId = `avatar-gradient-${index}`;
      const clipId = `avatar-clip-${index}`;
      return `<defs>${createAvatarGradientSvg(avatar.seed, gradientId)}<clipPath id="${clipId}"><circle cx="${centerX}" cy="${centerY}" r="28"/></clipPath></defs><g data-avatar="participant" data-avatar-renderer="app"><circle cx="${centerX}" cy="${centerY}" r="28" fill="url(#${gradientId})"/>${avatar.image ? `<image href="${avatar.image}" x="${centerX - 28}" y="${centerY - 28}" width="56" height="56" preserveAspectRatio="xMidYMid slice" clip-path="url(#${clipId})"/>` : ""}<circle cx="${centerX}" cy="${centerY}" r="30" fill="none" stroke="#f4f0e8" stroke-width="4"/><text x="${centerX}" y="${centerY + 7}" fill="#ffffff" fill-opacity="0.82" font-family="Arial, Helvetica, sans-serif" font-size="18" font-weight="700" text-anchor="middle" style="mix-blend-mode:overlay">${escapeXml(avatar.label)}</text></g>`;
    })
    .reverse()
    .join("");
}

function estimateSansTextWidth(value: string, fontSize: number) {
  return Array.from(value).reduce((width, character) => {
    if (/\s/.test(character)) return width + fontSize * 0.28;
    if (/[ilI1.,'`]/.test(character)) return width + fontSize * 0.3;
    if (/[MW@%]/.test(character)) return width + fontSize * 0.82;
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
  const title = wrapText(clampText(input.title, 96), 25, 3);
  const description = wrapText(clampText(input.description, 150), 55, 2);
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
        `<text x="86" y="${titleStartY + index * 86}" fill="#181613" font-family="Georgia, 'Times New Roman', serif" font-size="76" font-weight="700">${escapeXml(line)}</text>`,
    )
    .join("")}
  ${description
    .map(
      (line, index) =>
        `<text x="90" y="${descriptionStartY + index * 42}" fill="#57534e" font-family="Arial, Helvetica, sans-serif" font-size="32" font-weight="500">${escapeXml(line)}</text>`,
    )
    .join("")}
  <text x="86" y="552" fill="#756b5d" font-family="Arial, Helvetica, sans-serif" font-size="26" font-weight="600">${escapeXml(meta || "anarlog.so")}</text>
  ${createAnarlogWordmark({ x: 962, y: 516, width: 152 })}
</svg>`;
}

export function createSharedNoteOgSvg(
  input: SharedNoteOgImageInput,
  avatarImages: string[] = [],
) {
  const normalizedTitle = clampText(input.title, 120) || "Shared note";
  const titleFontSize = normalizedTitle.length > 72 ? 64 : 76;
  const title = wrapText(
    normalizedTitle,
    normalizedTitle.length > 72 ? 31 : 27,
    3,
  );
  const participantPresentation = createSharedNoteParticipantPresentation(
    input.participants ?? [],
  );
  const participantSummary = clampText(participantPresentation.label, 42);
  const avatarParticipants = participantPresentation.avatarParticipants;
  const summary = clampText(input.summary, 72);
  const titleStartY = title.length === 1 ? 152 : title.length === 2 ? 116 : 90;
  const titleEndY = titleStartY + (title.length - 1) * 82;
  const summaryY = titleEndY + 58;
  const date = formatDate(input.meetingAt) || "Date unavailable";
  const footerCenterY = 500;
  const avatarCount = avatarParticipants.length;
  const participantX = avatarCount ? 100 + (avatarCount - 1) * 42 + 44 : 72;
  const estimatedParticipantTextWidth = estimateSansTextWidth(
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
  <rect width="${OG_WIDTH}" height="${OG_HEIGHT}" fill="#f4f0e8"/>
  <path d="M72 424 H1128" stroke="#cfc6ba" stroke-width="2"/>
  ${title
    .map(
      (line, index) =>
        `<text x="72" y="${titleStartY + index * 82}" fill="#181613" font-family="Georgia, 'Times New Roman', serif" font-size="${titleFontSize}" font-weight="700">${escapeXml(line)}</text>`,
    )
    .join("")}
  ${summary ? `<text data-summary="meeting" x="72" y="${summaryY}" fill="#57534e" font-family="Arial, Helvetica, sans-serif" font-size="31" font-weight="500">${escapeXml(summary)}</text>` : ""}
  ${createParticipantAvatarStack(avatarParticipants, avatarImages, footerCenterY)}
  <text x="${participantX}" y="${footerCenterY + 9}"${participantTextLength} fill="#37322d" font-family="Arial, Helvetica, sans-serif" font-size="27" font-weight="600">${escapeXml(participantSummary)}</text>
  <circle cx="${separatorX}" cy="${footerCenterY}" r="3" fill="#9d9387"/>
  <text x="${dateX}" y="${footerCenterY + 9}" fill="#57534e" font-family="Arial, Helvetica, sans-serif" font-size="27" font-weight="500">${escapeXml(date)}</text>
  ${createAnarlogWordmark({ x: 963, y: 477, width: 165 })}
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
