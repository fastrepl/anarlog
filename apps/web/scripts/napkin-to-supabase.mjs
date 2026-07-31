#!/usr/bin/env node
import { createClient } from "@supabase/supabase-js";
import { readFile } from "node:fs/promises";
import { basename, extname } from "node:path";

const NAPKIN_API_BASE = "https://api.napkin.ai";
const BLOG_BUCKET = "blog";
const DEFAULT_STYLE_ID = "CDQPRVVJCSTPRBBCD5Q6AWSDE8S0";
const DEFAULT_LANGUAGE = "en-US";
const DEFAULT_POLL_INTERVAL_MS = 5000;
const DEFAULT_TIMEOUT_MS = 180000;

const MIME_BY_FORMAT = {
  png: "image/png",
  svg: "image/svg+xml",
  ppt: "application/vnd.openxmlformats-officedocument.presentationml.presentation",
};

function parseArgs(argv) {
  const args = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--") continue;
    if (!arg.startsWith("--")) {
      throw new Error(`Unexpected positional argument: ${arg}`);
    }

    const [rawKey, inlineValue] = arg.slice(2).split("=", 2);
    const key = rawKey.replace(/-([a-z])/g, (_, char) => char.toUpperCase());

    if (inlineValue !== undefined) {
      args[key] = inlineValue;
      continue;
    }

    const next = argv[index + 1];
    if (!next || next.startsWith("--")) {
      args[key] = true;
      continue;
    }

    args[key] = next;
    index += 1;
  }
  return args;
}

function usage() {
  return `Usage:
  pnpm --dir apps/web exec node scripts/napkin-to-supabase.mjs \\
    --slug meeting-minutes-software \\
    --filename meeting-minutes-workflow.png \\
    --content-file /tmp/figure-prompt.txt \\
    --context "Anarlog blog figure for private, bot-free meeting notes"

Required:
  --slug           Blog article slug. Uploads to articles/<slug>/<filename>.
  --filename       Output filename. Extension should match --format.
  --content        Text to visualize, or use --content-file.
  --content-file   File containing text to visualize.

Useful options:
  --format png|svg|ppt              Defaults from filename extension, then png.
  --context "..."                   Additional generation context.
  --style-id <id>                   Napkin style/brand ID. Defaults to Anarlog brand.
  --visual-query flowchart|timeline Optional layout hint.
  --orientation auto|horizontal|vertical|square
  --width 1200                      PNG width hint.
  --height 800                      PNG height hint. Width takes precedence in Napkin.
  --language en-US                  Defaults to en-US.
  --number-of-visuals 1             1-4. Defaults to 1.
  --file-index 0                    Which generated file to upload. Defaults to 0.
  --upsert                          Replace existing Supabase object.
  --dry-run                         Create/download nothing; print request and target.

Environment:
  NAPKIN_API_TOKEN
  SUPABASE_URL
  SUPABASE_SERVICE_ROLE_KEY
`;
}

function requireEnv(name) {
  const value = process.env[name];
  if (!value) throw new Error(`Missing required environment variable: ${name}`);
  return value;
}

function normalizeSlug(value) {
  const slug = String(value || "")
    .trim()
    .replace(/^\/+|\/+$/g, "");
  if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(slug)) {
    throw new Error(`Invalid slug: ${value}`);
  }
  return slug;
}

function normalizeFilename(value, format) {
  const name = basename(String(value || "").trim());
  if (!name || name === "." || name === "..") {
    throw new Error("Invalid filename");
  }
  if (!/^[a-z0-9][a-z0-9._-]*\.(png|svg|ppt)$/i.test(name)) {
    throw new Error("Filename must be a simple .png, .svg, or .ppt file");
  }
  const extension = extname(name).slice(1).toLowerCase();
  if (extension !== format) {
    throw new Error(
      `Filename extension .${extension} does not match format ${format}`,
    );
  }
  return name;
}

function normalizeFormat(args) {
  const explicit = args.format && String(args.format).toLowerCase();
  const inferred =
    args.filename && extname(String(args.filename)).slice(1).toLowerCase();
  const format = explicit || inferred || "png";
  if (!["png", "svg", "ppt"].includes(format)) {
    throw new Error(`Unsupported format: ${format}`);
  }
  return format;
}

function asOptionalInteger(value, name) {
  if (value === undefined || value === true || value === "") return undefined;
  const parsed = Number(value);
  if (!Number.isInteger(parsed)) throw new Error(`${name} must be an integer`);
  return parsed;
}

async function readContent(args) {
  if (args.content && args.contentFile) {
    throw new Error("Use either --content or --content-file, not both");
  }
  if (args.content) return String(args.content).trim();
  if (args.contentFile)
    return (await readFile(String(args.contentFile), "utf8")).trim();
  throw new Error("Missing --content or --content-file");
}

async function napkinFetch(pathOrUrl, options = {}) {
  const token = requireEnv("NAPKIN_API_TOKEN");
  const url = pathOrUrl.startsWith("http")
    ? pathOrUrl
    : `${NAPKIN_API_BASE}${pathOrUrl}`;
  const response = await fetch(url, {
    ...options,
    headers: {
      Authorization: `Bearer ${token}`,
      ...(options.body ? { "Content-Type": "application/json" } : {}),
      ...options.headers,
    },
  });

  if (!response.ok) {
    const body = await response.text();
    throw new Error(`Napkin request failed ${response.status}: ${body}`);
  }

  return response;
}

async function createNapkinVisual(request) {
  const response = await napkinFetch("/v1/visual", {
    method: "POST",
    body: JSON.stringify(request),
  });
  const json = await response.json();
  const id = json.id || json.request_id || json.requestId;
  if (!id) {
    throw new Error(
      `Napkin create response did not include request id: ${JSON.stringify(json)}`,
    );
  }
  return { id, json };
}

async function pollNapkinVisual(requestId, timeoutMs) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    const response = await napkinFetch(`/v1/visual/${requestId}/status`);
    const json = await response.json();

    if (json.status === "completed") return json;
    if (json.status === "failed") {
      throw new Error(
        `Napkin generation failed: ${JSON.stringify(json.error || json)}`,
      );
    }

    await new Promise((resolve) =>
      setTimeout(resolve, DEFAULT_POLL_INTERVAL_MS),
    );
  }

  throw new Error(`Napkin generation timed out after ${timeoutMs}ms`);
}

async function downloadGeneratedFile(file) {
  if (!file?.url) throw new Error("Selected generated file has no URL");
  const response = await napkinFetch(file.url);
  const bytes = new Uint8Array(await response.arrayBuffer());
  return {
    bytes,
    contentType: response.headers.get("content-type"),
  };
}

function getSupabaseClient() {
  return createClient(
    requireEnv("SUPABASE_URL"),
    requireEnv("SUPABASE_SERVICE_ROLE_KEY"),
    {
      auth: {
        persistSession: false,
        autoRefreshToken: false,
      },
    },
  );
}

async function uploadToSupabase({ storagePath, bytes, contentType, upsert }) {
  const supabase = getSupabaseClient();
  const { error } = await supabase.storage
    .from(BLOG_BUCKET)
    .upload(storagePath, bytes, {
      cacheControl: "31536000",
      contentType,
      upsert,
    });

  if (error) throw error;

  const { data } = supabase.storage.from(BLOG_BUCKET).getPublicUrl(storagePath);
  return data.publicUrl;
}

async function checkPublicUrl(url) {
  const response = await fetch(url, { method: "HEAD" });
  if (!response.ok) {
    throw new Error(
      `Uploaded public URL check failed ${response.status}: ${url}`,
    );
  }
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) {
    console.log(usage());
    return;
  }

  const format = normalizeFormat(args);
  const slug = normalizeSlug(args.slug);
  const filename = normalizeFilename(args.filename, format);
  const content = await readContent(args);
  const storagePath = `articles/${slug}/${filename}`;
  const fileIndex = asOptionalInteger(args.fileIndex, "file-index") ?? 0;
  const numberOfVisuals =
    asOptionalInteger(args.numberOfVisuals, "number-of-visuals") ?? 1;
  const width = asOptionalInteger(args.width, "width");
  const height = asOptionalInteger(args.height, "height");
  const timeoutMs =
    asOptionalInteger(args.timeoutMs, "timeout-ms") ?? DEFAULT_TIMEOUT_MS;

  const request = {
    format,
    content,
    language: args.language || DEFAULT_LANGUAGE,
    style_id: args.styleId || DEFAULT_STYLE_ID,
    number_of_visuals: numberOfVisuals,
    text_extraction_mode: args.textExtractionMode || "auto",
    sort_strategy: args.sortStrategy || "relevance",
  };

  if (args.context) request.context = String(args.context);
  if (args.visualQuery) request.visual_query = String(args.visualQuery);
  if (args.orientation) request.orientation = String(args.orientation);
  if (width !== undefined) request.width = width;
  if (height !== undefined) request.height = height;
  if (args.transparentBackground) request.transparent_background = true;
  if (args.colorMode) request.color_mode = String(args.colorMode);

  const target = {
    bucket: BLOG_BUCKET,
    storagePath,
    mediaUrl: `/api/assets/blog/${storagePath}`,
    format,
    contentType: MIME_BY_FORMAT[format],
    upsert: Boolean(args.upsert),
  };

  if (args.dryRun) {
    console.log(JSON.stringify({ request, target }, null, 2));
    return;
  }

  const created = await createNapkinVisual(request);
  console.error(`Napkin request created: ${created.id}`);

  const completed = await pollNapkinVisual(created.id, timeoutMs);
  const files = completed.generated_files || [];
  if (!files[fileIndex]) {
    throw new Error(
      `No generated file at index ${fileIndex}; received ${files.length} file(s)`,
    );
  }

  const downloaded = await downloadGeneratedFile(files[fileIndex]);
  const contentType = downloaded.contentType || MIME_BY_FORMAT[format];
  const publicUrl = await uploadToSupabase({
    storagePath,
    bytes: downloaded.bytes,
    contentType,
    upsert: Boolean(args.upsert),
  });
  await checkPublicUrl(publicUrl);

  console.log(
    JSON.stringify(
      {
        napkinRequestId: created.id,
        selectedFile: files[fileIndex],
        bucket: BLOG_BUCKET,
        storagePath,
        publicUrl,
        mediaUrl: `/api/assets/blog/${storagePath}`,
        bytes: downloaded.bytes.length,
        contentType,
      },
      null,
      2,
    ),
  );
}

main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});
