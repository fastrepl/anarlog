import { createHash } from "node:crypto";
import { readdir, readFile, writeFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";
import { parseArgs } from "node:util";

const ASSET_ROOT = new URL("../../desktop/public/assets/", import.meta.url);
const MANIFEST = new URL(
  "../../desktop/src/shared/static-assets.manifest.json",
  import.meta.url,
);
const MIME_TYPES = {
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  svg: "image/svg+xml",
  webp: "image/webp",
  gif: "image/gif",
  mp4: "video/mp4",
};

export async function collectStaticAssets(root = ASSET_ROOT, prefix = "") {
  const entries = await readdir(root, { withFileTypes: true });
  const assets = [];
  for (const entry of entries.sort((a, b) =>
    a.name.localeCompare(b.name, "en"),
  )) {
    const path = `${prefix}${entry.name}`;
    const file = new URL(encodeURIComponent(entry.name), root);
    if (entry.isDirectory()) {
      assets.push(
        ...(await collectStaticAssets(new URL(`${file.href}/`), `${path}/`)),
      );
    } else if (entry.isFile()) {
      const contentType = MIME_TYPES[entry.name.split(".").at(-1)];
      if (!contentType) throw new Error(`Unsupported asset type: ${path}`);
      const bytes = await readFile(file);
      const digest = createHash("sha256").update(bytes).digest("hex");
      assets.push({
        local: `/assets/${path}`,
        object: `desktop/${digest}/${path}`,
        bytes,
        contentType,
      });
    } else {
      throw new Error(`Asset must be a regular file: ${path}`);
    }
  }
  return assets;
}

function manifestFor(assets) {
  return `${JSON.stringify(Object.fromEntries(assets.map(({ local, object }) => [local, object])), null, 2)}\n`;
}

export async function publishStaticAssets(
  assets,
  { supabaseUrl, serviceRoleKey, fetchImpl = fetch },
) {
  const base = new URL(supabaseUrl);
  if (base.protocol !== "https:")
    throw new Error("SUPABASE_URL must use HTTPS");
  if (!serviceRoleKey) throw new Error("SUPABASE_SERVICE_ROLE_KEY is required");
  const headers = {
    authorization: `Bearer ${serviceRoleKey}`,
    apikey: serviceRoleKey,
  };
  const bucketResponse = await fetchImpl(
    new URL("/storage/v1/bucket/public_images", base),
    { headers },
  );
  if (!bucketResponse.ok || !(await bucketResponse.json()).public) {
    throw new Error(
      "The existing public_images bucket must be public and accessible",
    );
  }
  for (const asset of assets) {
    const object = asset.object.split("/").map(encodeURIComponent).join("/");
    const upload = await fetchImpl(
      new URL(`/storage/v1/object/public_images/${object}`, base),
      {
        method: "POST",
        headers: {
          ...headers,
          "content-type": asset.contentType,
          "cache-control": "max-age=31536000",
          "x-upsert": "false",
        },
        body: asset.bytes,
      },
    );
    if (!upload.ok && upload.status !== 409 && upload.status !== 400) {
      throw new Error(
        `Upload failed for ${asset.local} (HTTP ${upload.status})`,
      );
    }
    // A retry may encounter an existing immutable object. Verify its bytes rather than overwriting it.
    const download = await fetchImpl(
      new URL(`/storage/v1/object/public/public_images/${object}`, base),
    );
    if (!download.ok)
      throw new Error(
        `Public download failed for ${asset.local} (HTTP ${download.status})`,
      );
    const actual = createHash("sha256")
      .update(Buffer.from(await download.arrayBuffer()))
      .digest("hex");
    if (actual !== asset.object.split("/")[1])
      throw new Error(`Public asset checksum mismatch: ${asset.local}`);
  }
}

async function main() {
  const { values } = parseArgs({
    options: {
      upload: { type: "boolean" },
      manifest: { type: "boolean" },
      check: { type: "boolean" },
    },
  });
  const assets = await collectStaticAssets();
  const manifest = manifestFor(assets);
  if (values.check && (await readFile(MANIFEST, "utf8")) !== manifest) {
    throw new Error(
      "Static asset manifest is stale; run pnpm -F @anlg/web assets:manifest",
    );
  }
  if (values.upload) {
    const supabaseUrl = process.env.SUPABASE_URL;
    const serviceRoleKey = process.env.SUPABASE_SERVICE_ROLE_KEY;
    if (!supabaseUrl || !serviceRoleKey)
      throw new Error(
        "Set SUPABASE_URL and SUPABASE_SERVICE_ROLE_KEY to upload assets",
      );
    await publishStaticAssets(assets, { supabaseUrl, serviceRoleKey });
  }
  if (values.manifest) await writeFile(MANIFEST, manifest);
  console.log(
    `${values.upload ? "Published and verified" : "Prepared"} ${assets.length} assets (${(assets.reduce((sum, asset) => sum + asset.bytes.length, 0) / 1024 / 1024).toFixed(1)} MiB) for https://static.anarlog.so/desktop/`,
  );
}

if (
  process.argv[1] &&
  pathToFileURL(process.argv[1]).href === import.meta.url
) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
