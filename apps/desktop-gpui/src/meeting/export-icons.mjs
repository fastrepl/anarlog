// Regenerates the meeting surfaces' checked-in Hugeicons SVGs (the shipping app's icon set):
// node apps/desktop-gpui/src/meeting/export-icons.mjs
import {
  AlertCircleIcon,
  ArrowDown02Icon,
  ArrowExpandIcon,
  ArrowShrinkIcon,
  ArrowUp02Icon,
  AudioWaveformIcon,
  FolderOpenIcon,
  PauseIcon,
  PlayIcon,
  RefreshIcon,
  SquareIcon,
} from "@hugeicons/core-free-icons";
import { writeFileSync } from "node:fs";

const escape = (value) =>
  String(value)
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;");
for (const [name, data] of Object.entries({
  AlertCircleIcon,
  ArrowDown02Icon,
  ArrowExpandIcon,
  ArrowShrinkIcon,
  ArrowUp02Icon,
  AudioWaveformIcon,
  FolderOpenIcon,
  PauseIcon,
  PlayIcon,
  RefreshIcon,
  SquareIcon,
})) {
  const elements = data
    .map(
      ([tag, attrs]) =>
        `<${tag} ${Object.entries(attrs)
          .filter(([key]) => key !== "key")
          .map(
            ([key, value]) =>
              `${key.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)}="${escape(value)}"`,
          )
          .join(" ")} />`,
    )
    .join("");
  writeFileSync(
    new URL(`./icons/${name}.svg`, import.meta.url),
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">${elements}</svg>\n`,
  );
}
