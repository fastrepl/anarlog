import assert from "node:assert/strict";
import test from "node:test";
import sharp from "sharp";

import { renderSharedNoteOgImage } from "./og-image.ts";

test("renders a large social image for a shared note", async () => {
  const response = await renderSharedNoteOgImage({
    title: "Sprint retro & planning",
    description:
      "A concise recap of decisions, action items, and what the team is doing next.",
    publishedAt: "2026-07-03T12:00:00Z",
  });
  const image = sharp(Buffer.from(await response.arrayBuffer()));
  const metadata = await image.metadata();

  assert.equal(response.headers.get("Content-Type"), "image/png");
  assert.equal(
    response.headers.get("Cache-Control"),
    "public, max-age=0, s-maxage=60",
  );
  assert.equal(metadata.format, "png");
  assert.equal(metadata.width, 1200);
  assert.equal(metadata.height, 630);
});
