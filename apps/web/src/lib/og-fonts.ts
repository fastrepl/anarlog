import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const FONT_MARKER_FILENAME = "Redaction-Regular.otf";

export const SERIF_FONT_FAMILY = "'Redaction', Georgia, serif";
export const SANS_FONT_FAMILY = "'SF Pro Text', Arial, Helvetica, sans-serif";

function resolveOgFontsDir() {
  const candidates = [
    join(fileURLToPath(new URL("../../public/fonts", import.meta.url))),
    join(process.cwd(), "public", "fonts"),
    join(process.cwd(), "apps", "web", "public", "fonts"),
    "/var/task/public/fonts",
    "/var/task/apps/web/public/fonts",
  ];
  return candidates.find((dir) => existsSync(join(dir, FONT_MARKER_FILENAME)));
}

function configureOgFonts() {
  const fontsDir = resolveOgFontsDir();
  if (!fontsDir) return;

  const cacheDir = join(tmpdir(), "anarlog-fontconfig-cache");
  mkdirSync(cacheDir, { recursive: true });
  const runtimeConfigPath = join(tmpdir(), "anarlog-og-fonts.conf");
  writeFileSync(
    runtimeConfigPath,
    `<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
<fontconfig>
  <include ignore_missing="yes">/etc/fonts/fonts.conf</include>
  <dir>${fontsDir}</dir>
  <cachedir>${cacheDir}</cachedir>
</fontconfig>
`,
  );
  process.env.FONTCONFIG_FILE = runtimeConfigPath;
}

configureOgFonts();
