// Regenerates the editor's checked-in Hugeicons SVGs (the shipping app's icon set):
// node apps/desktop-gpui/src/editor/export-icons.mjs
import {
  AlertCircleIcon,
  AppWindowIcon,
  ArrowDown01Icon,
  ArrowDown02Icon,
  ArrowRight01Icon,
  ArrowUp02Icon,
  AudioWaveformIcon,
  Building03Icon,
  Calendar04Icon,
  CheckIcon,
  CheckmarkCircle02Icon,
  CodeIcon,
  FileDownloadIcon,
  FileTextIcon,
  Heading01Icon,
  Heading02Icon,
  Heading03Icon,
  HeartIcon,
  HighlighterIcon,
  HistoryIcon,
  LeftToRightListBulletIcon,
  LeftToRightListNumberIcon,
  ListChecksIcon,
  LockKeyholeOpenIcon,
  LockPasswordIcon,
  Mic01Icon,
  MicOff01Icon,
  MinusIcon,
  MoreHorizontalIcon,
  NoteIcon,
  PencilEdit01Icon,
  PinIcon,
  PlusIcon,
  QuotesIcon,
  RefreshIcon,
  RotateClockwiseIcon,
  Search01Icon,
  Share08Icon,
  SparkleIcon,
  SquareIcon,
  TextAlignLeftIcon,
  TextBoldIcon,
  TextIcon,
  TextItalicIcon,
  TextStrikethroughIcon,
  TextUnderlineIcon,
  TrashIcon,
  UserIcon,
  XIcon,
} from "@hugeicons/core-free-icons";
import { writeFileSync } from "node:fs";

const escape = (value) =>
  String(value)
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;");
for (const [name, data] of Object.entries({
  AlertCircleIcon,
  AppWindowIcon,
  ArrowDown01Icon,
  ArrowDown02Icon,
  ArrowRight01Icon,
  ArrowUp02Icon,
  AudioWaveformIcon,
  Building03Icon,
  Calendar04Icon,
  CheckIcon,
  CheckmarkCircle02Icon,
  CodeIcon,
  FileDownloadIcon,
  FileTextIcon,
  Heading01Icon,
  Heading02Icon,
  Heading03Icon,
  HeartIcon,
  HighlighterIcon,
  HistoryIcon,
  LeftToRightListBulletIcon,
  LeftToRightListNumberIcon,
  ListChecksIcon,
  LockKeyholeOpenIcon,
  LockPasswordIcon,
  Mic01Icon,
  MicOff01Icon,
  MinusIcon,
  MoreHorizontalIcon,
  NoteIcon,
  PencilEdit01Icon,
  PinIcon,
  PlusIcon,
  QuotesIcon,
  RefreshIcon,
  RotateClockwiseIcon,
  Search01Icon,
  Share08Icon,
  SparkleIcon,
  SquareIcon,
  TextAlignLeftIcon,
  TextBoldIcon,
  TextIcon,
  TextItalicIcon,
  TextStrikethroughIcon,
  TextUnderlineIcon,
  TrashIcon,
  UserIcon,
  XIcon,
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
