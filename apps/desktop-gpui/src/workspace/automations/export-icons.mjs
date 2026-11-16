// Writes the automations Hugeicons that the catalog icons do not embed.
// Run with Node from a checkout whose `node_modules` has `@hugeicons/core-free-icons`.
// The brand logos are Iconify's `logos:*` set as shipping renders them; `MarkdownMark` is
// `apps/desktop/public/assets/markdown-mark.svg`.
import {
  ArrowDown01Icon,
  ArrowRight02Icon,
  EyeIcon,
  FloppyDiskIcon,
  FolderOpenIcon,
  LoaderCircleIcon,
  PlayIcon,
  TrashIcon,
} from "@hugeicons/core-free-icons";
import { writeFileSync } from "node:fs";

const escape = (value) =>
  String(value)
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;");
for (const [name, data] of Object.entries({
  ArrowDown01Icon,
  ArrowRight02Icon,
  EyeIcon,
  FloppyDiskIcon,
  FolderOpenIcon,
  LoaderCircleIcon,
  PlayIcon,
  TrashIcon,
})) {
  const elements = data
    .map(
      ([tag, attrs]) =>
        `<${tag} ${Object.entries(attrs)
          .filter(([key]) => key !== "key")
          .map(([key, value]) => [key, key === "strokeWidth" ? "1.5" : value])
          .map(
            ([key, value]) =>
              `${key.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)}="${escape(value)}"`,
          )
          .join(" ")} />`,
    )
    .join("");
  writeFileSync(
    new URL(`icons/${name}.svg`, import.meta.url),
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">${elements}</svg>\n`,
  );
}
