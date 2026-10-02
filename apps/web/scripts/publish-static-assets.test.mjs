import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { pathToFileURL } from "node:url";

import {
  collectStaticAssets,
  publishStaticAssets,
} from "./publish-static-assets.mjs";

test("publishes immutable media with public byte verification and detects corrupt retry objects", async (t) => {
  const folder = await mkdtemp(join(tmpdir(), "anlg-static-assets-"));
  t.after(() => rm(folder, { recursive: true, force: true }));
  await writeFile(join(folder, "icon space.svg"), "original");
  const assets = await collectStaticAssets(pathToFileURL(`${folder}/`));
  await writeFile(join(folder, "icon space.svg"), "changed");
  const changed = await collectStaticAssets(pathToFileURL(`${folder}/`));
  assert.notEqual(assets[0].object, changed[0].object);
  let corrupt = false;
  const fetchImpl = async (url, options = {}) => {
    if (url.pathname === "/storage/v1/bucket/public_images")
      return Response.json({ public: true });
    assert.ok(url.pathname.endsWith("/icon%20space.svg"));
    if (options.method === "POST") {
      assert.equal(options.headers["x-upsert"], "false");
      assert.equal(options.headers["content-type"], "image/svg+xml");
      assert.equal(options.headers["cache-control"], "max-age=31536000");
      return new Response(null, { status: 409 });
    }
    assert.equal(options.headers, undefined);
    return new Response(corrupt ? "corrupted" : "original");
  };
  const config = {
    supabaseUrl: "https://example.supabase.co",
    serviceRoleKey: "test-key",
    fetchImpl,
  };
  await publishStaticAssets(assets, config);
  corrupt = true;
  await assert.rejects(
    publishStaticAssets(assets, config),
    /checksum mismatch/,
  );
});
