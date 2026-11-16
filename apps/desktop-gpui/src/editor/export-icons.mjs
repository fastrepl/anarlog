// Regenerates the editor's checked-in Hugeicons SVGs (the shipping app's icon set):
// node apps/desktop-gpui/src/editor/export-icons.mjs
import {
  AppWindowIcon,
  ArrowRight01Icon,
  AudioWaveformIcon,
  Calendar04Icon,
  CheckIcon,
  FileDownloadIcon,
  FileTextIcon,
  HistoryIcon,
  LockKeyholeOpenIcon,
  LockPasswordIcon,
  Mic01Icon,
  MicOff01Icon,
  MoreHorizontalIcon,
  PinIcon,
  Share08Icon,
  SparkleIcon,
  SquareIcon,
  TextAlignLeftIcon,
  TrashIcon,
} from "@hugeicons/core-free-icons";
import { writeFileSync } from "node:fs";

const escape = (value) =>
  String(value)
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;");
for (const [name, data] of Object.entries({
  AppWindowIcon,
  ArrowRight01Icon,
  AudioWaveformIcon,
  Calendar04Icon,
  CheckIcon,
  FileDownloadIcon,
  FileTextIcon,
  HistoryIcon,
  LockKeyholeOpenIcon,
  LockPasswordIcon,
  Mic01Icon,
  MicOff01Icon,
  MoreHorizontalIcon,
  PinIcon,
  Share08Icon,
  SparkleIcon,
  SquareIcon,
  TextAlignLeftIcon,
  TrashIcon,
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
