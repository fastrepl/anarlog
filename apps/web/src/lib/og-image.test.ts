import "./og-fonts.ts";

import assert from "node:assert/strict";
import test from "node:test";
import sharp from "sharp";

import {
  createBlogOgSvg,
  createSharedNoteOgSvg,
  renderBlogOgImage,
  renderSharedNoteOgImage,
} from "./og-image.ts";

test("renders blog metadata into a post-specific image", async () => {
  const svg = createBlogOgSvg({
    title: "How to take better meeting notes",
    description: "A practical guide for focused meetings.",
    date: "2026-08-06T00:00:00Z",
    author: "John & team",
  });

  assert.match(svg, /How to take better/);
  assert.match(svg, /John &amp; team - August 6, 2026/);
  assert.doesNotMatch(svg, />Anarlog<\/text>/);
  assert.doesNotMatch(svg, />Blog<\/text>/);
  assert.doesNotMatch(svg, />anarlog blog<\/text>/);
  assert.doesNotMatch(svg, /anarlog\.so/);
  assert.match(svg, /font-family="'Redaction', Georgia, serif"/);
  assert.match(svg, /data-wordmark="anarlog"/);
  assert.match(svg, /<rect width="1200" height="630" fill="#ffffff"\/>/);
  assert.doesNotMatch(svg, /<rect x=/);
  assert.match(createBlogOgSvg({ title: "Hi" }), />anarlog<\/text>/);

  const response = await renderBlogOgImage({ title: "Dynamic blog post" });
  assert.match(process.env.FONTCONFIG_FILE ?? "", /anarlog-og-fonts\.conf$/);
  const metadata = await sharp(
    Buffer.from(await response.arrayBuffer()),
  ).metadata();
  assert.equal(metadata.width, 1200);
  assert.equal(metadata.height, 630);
});

test("normalizes shared note metadata", () => {
  const svg = createSharedNoteOgSvg({
    title: "   ",
    summary: "The team aligned on launch scope and the remaining blockers.",
    participants: [
      " John  Jeong ",
      "John Jeong",
      "Artem",
      "Sungbin Jo",
      "Yujong Lee",
      "Julie",
    ],
    meetingAt: "2026-07-02T23:30:00Z",
  });

  assert.match(svg, />Shared note<\/text>/);
  assert.match(
    svg,
    />The team aligned on launch scope and the remaining blockers\.<\/text>/,
  );
  assert.match(svg, /John, Artem \+3 more/);
  assert.doesNotMatch(svg, />\+3<\/text>/);
  assert.equal(svg.match(/data-avatar=/g)?.length, 5);
  assert.equal(svg.match(/data-avatar-renderer="app"/g)?.length, 5);
  assert.ok(
    svg.indexOf('id="avatar-gradient-0"') >
      svg.indexOf('id="avatar-gradient-1"'),
  );
  assert.match(svg, />July 2, 2026<\/text>/);
  assert.doesNotMatch(svg, /cx="592"/);
  assert.match(svg, /data-wordmark="anarlog"/);
  assert.match(svg, /font-family="'Redaction', Georgia, serif"/);
  assert.match(
    svg,
    /font-family="'SF Pro Text', Arial, Helvetica, sans-serif"/,
  );
  assert.doesNotMatch(svg, /Redaction 70/);
  assert.doesNotMatch(svg, /anarlog\.so/);
  assert.doesNotMatch(svg, /PARTICIPANTS|WHEN|SHARED NOTE|Read on anarlog\.so/);
  assert.doesNotMatch(svg, /filter="url\(#shadow\)"/);
});

test("renders a large social image for a shared note", async () => {
  const response = await renderSharedNoteOgImage({
    title: "Sprint retro & planning",
    summary: "A review of the sprint and next week's priorities.",
    participants: ["John Jeong", "Sungbin Jo"],
    meetingAt: "2026-07-03T12:00:00Z",
  });
  const png = Buffer.from(await response.arrayBuffer());
  const image = sharp(png);
  const metadata = await image.metadata();

  assert.equal(response.headers.get("Content-Type"), "image/png");
  assert.equal(
    response.headers.get("Cache-Control"),
    "public, max-age=0, s-maxage=60",
  );
  assert.equal(metadata.format, "png");
  assert.equal(metadata.width, 1200);
  assert.equal(metadata.height, 630);
  assert.ok(
    (await countDarkPixels(png, {
      left: 60,
      top: 70,
      width: 700,
      height: 120,
    })) > 1500,
    "expected the note title to rasterize as filled glyphs",
  );
  assert.ok(
    (await countDarkPixels(png, {
      left: 240,
      top: 470,
      width: 520,
      height: 50,
    })) > 400,
    "expected participant and date text to rasterize as filled glyphs",
  );
});

test("caps participant avatars in crowded shared-note previews", () => {
  const svg = createSharedNoteOgSvg({
    title: "Large meeting",
    participants: Array.from({ length: 32 }, (_, index) => `Person ${index}`),
    meetingAt: "2026-07-03T12:00:00Z",
  });

  assert.equal(svg.match(/data-avatar=/g)?.length, 5);
  assert.match(svg, /Person, Person \+30 more/);
});

async function countDarkPixels(
  png: Buffer,
  region: { left: number; top: number; width: number; height: number },
) {
  const { data, info } = await sharp(png)
    .extract(region)
    .raw()
    .toBuffer({ resolveWithObject: true });
  let dark = 0;
  for (let index = 0; index < data.length; index += info.channels) {
    const red = data[index];
    const green = data[index + 1];
    const blue = data[index + 2];
    if (red < 80 && green < 80 && blue < 80) dark += 1;
  }
  return dark;
}
