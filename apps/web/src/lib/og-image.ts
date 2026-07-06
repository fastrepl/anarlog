import sharp from "sharp";

const OG_WIDTH = 1200;
const OG_HEIGHT = 630;
const CACHE_CONTROL =
  "public, max-age=0, s-maxage=86400, stale-while-revalidate=604800";

type BlogOgImageInput = {
  title: string;
  description?: string;
  date?: string;
  author?: string;
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

export async function renderBlogOgImage(input: BlogOgImageInput) {
  const svg = createBlogOgSvg(input);
  const png = await sharp(Buffer.from(svg)).png().toBuffer();

  return new Response(new Uint8Array(png), {
    headers: {
      "Cache-Control": CACHE_CONTROL,
      "Content-Type": "image/png",
    },
  });
}
