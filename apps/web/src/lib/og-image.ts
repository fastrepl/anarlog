import sharp from "sharp";

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
  description?: string;
  publishedAt?: string;
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
    year: "numeric",
  });
}

function createBlogOgSvg(input: BlogOgImageInput) {
  const title = wrapText(clampText(input.title, 96), 25, 3);
  const description = wrapText(clampText(input.description, 150), 55, 2);
  const meta = [input.author, formatDate(input.date)]
    .filter(Boolean)
    .join(" - ");
  const titleStartY = title.length === 1 ? 266 : title.length === 2 ? 226 : 190;
  const descriptionStartY = titleStartY + title.length * 86 + 36;

  return `<?xml version="1.0" encoding="UTF-8"?>
<svg width="${OG_WIDTH}" height="${OG_HEIGHT}" viewBox="0 0 ${OG_WIDTH} ${OG_HEIGHT}" fill="none" xmlns="http://www.w3.org/2000/svg">
  <rect width="${OG_WIDTH}" height="${OG_HEIGHT}" fill="#f2f1ef"/>
  <rect x="44" y="44" width="1112" height="542" rx="0" fill="#ffffff"/>
  <path d="M86 126 H1114" stroke="#d8d1c8" stroke-width="2"/>
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
  <text x="86" y="100" fill="#57534e" font-family="Arial, Helvetica, sans-serif" font-size="34" font-weight="700">Anarlog</text>
  <text x="1114" y="100" fill="#756b5d" font-family="Arial, Helvetica, sans-serif" font-size="24" text-anchor="end">Blog</text>
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
</svg>`;
}

function createSharedNoteOgSvg(input: SharedNoteOgImageInput) {
  const normalizedTitle = clampText(input.title || "Shared note", 110);
  const titleFontSize = normalizedTitle.length > 72 ? 62 : 72;
  const title = wrapText(
    normalizedTitle,
    normalizedTitle.length > 72 ? 30 : 27,
    3,
  );
  const description = wrapText(clampText(input.description, 190), 59, 2);
  const titleStartY = title.length === 1 ? 265 : title.length === 2 ? 226 : 192;
  const descriptionStartY = titleStartY + title.length * 76 + 22;
  const date = formatDate(input.publishedAt);

  return `<?xml version="1.0" encoding="UTF-8"?>
<svg width="${OG_WIDTH}" height="${OG_HEIGHT}" viewBox="0 0 ${OG_WIDTH} ${OG_HEIGHT}" fill="none" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <linearGradient id="background" x1="44" y1="26" x2="1150" y2="608" gradientUnits="userSpaceOnUse">
      <stop stop-color="#fff7dd"/>
      <stop offset="0.52" stop-color="#f4efe6"/>
      <stop offset="1" stop-color="#ece6dc"/>
    </linearGradient>
    <filter id="shadow" x="28" y="24" width="1144" height="594" filterUnits="userSpaceOnUse" color-interpolation-filters="sRGB">
      <feDropShadow dx="0" dy="18" stdDeviation="18" flood-color="#4f412d" flood-opacity="0.12"/>
    </filter>
  </defs>
  <rect width="${OG_WIDTH}" height="${OG_HEIGHT}" fill="url(#background)"/>
  <circle cx="1090" cy="74" r="164" fill="#ffe09d" fill-opacity="0.55"/>
  <circle cx="72" cy="584" r="154" fill="#ffffff" fill-opacity="0.55"/>
  <rect x="58" y="46" width="1084" height="538" rx="30" fill="#fffefa" filter="url(#shadow)"/>
  <rect x="58" y="46" width="12" height="538" rx="6" fill="#e0b83d"/>
  <path d="M112 142 H1088" stroke="#e8e0d4" stroke-width="2"/>
  <g>
    <rect x="108" y="82" width="42" height="42" rx="12" fill="#181613"/>
    <path d="M119 108 C124 94 135 94 139 108 C135 103 124 103 119 108 Z" fill="#ffe09d"/>
    <circle cx="129" cy="110" r="3" fill="#ffe09d"/>
    <text x="166" y="113" fill="#181613" font-family="Arial, Helvetica, sans-serif" font-size="30" font-weight="700">Anarlog</text>
  </g>
  <g>
    <rect x="914" y="83" width="174" height="40" rx="20" fill="#f4efe6"/>
    <circle cx="941" cy="103" r="5" fill="#d1a321"/>
    <text x="958" y="111" fill="#756b5d" font-family="Arial, Helvetica, sans-serif" font-size="20" font-weight="700" letter-spacing="1.2">PUBLIC NOTE</text>
  </g>
  ${title
    .map(
      (line, index) =>
        `<text x="110" y="${titleStartY + index * 76}" fill="#181613" font-family="Georgia, 'Times New Roman', serif" font-size="${titleFontSize}" font-weight="700">${escapeXml(line)}</text>`,
    )
    .join("")}
  ${description
    .map(
      (line, index) =>
        `<text x="114" y="${descriptionStartY + index * 39}" fill="#57534e" font-family="Arial, Helvetica, sans-serif" font-size="29" font-weight="500">${escapeXml(line)}</text>`,
    )
    .join("")}
  <path d="M112 501 H1088" stroke="#e8e0d4" stroke-width="2"/>
  <g fill="#756b5d" font-family="Arial, Helvetica, sans-serif" font-size="23" font-weight="600">
    <text x="112" y="548">${escapeXml(date ? `Published ${date}` : "Shared note")}</text>
    <text x="1088" y="548" text-anchor="end">Read on anarlog.so</text>
  </g>
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
  return renderOgImage(createSharedNoteOgSvg(input), SHARED_NOTE_CACHE_CONTROL);
}
