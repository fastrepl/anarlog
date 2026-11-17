// Exports the settings and onboarding Hugeicons (the same icons shipping uses) as SVGs.
// node apps/desktop-gpui/src/product/export-icons.mjs
import {
  AlertCircleIcon,
  ArrowDown01Icon,
  ArrowRight01Icon,
  ArrowRight02Icon,
  AudioWaveformIcon,
  BellIcon,
  BookOpen01Icon,
  Building03Icon,
  Camera01Icon,
  CancelCircleIcon,
  ChartLineData01Icon,
  CheckIcon,
  CheckmarkCircle02Icon,
  CloudOffIcon,
  CodeIcon,
  CopyIcon,
  CreditCardIcon,
  Cursor01Icon,
  Download01Icon,
  ElectricPlugsIcon,
  ExternalLinkIcon,
  Key01Icon,
  LoaderCircleIcon,
  LockPasswordIcon,
  Mic01Icon,
  MoreHorizontalIcon,
  PencilEdit01Icon,
  Plug01Icon,
  PlusIcon,
  RefreshIcon,
  Settings02Icon,
  SentIcon,
  ShieldCheckIcon,
  SparkleIcon,
  Sun01Icon,
  TrashIcon,
  TriangleAlertIcon,
  UserGroupIcon,
  UserIcon,
  Video01Icon,
  VolumeHighIcon,
  VolumeXIcon,
  XIcon,
} from "@hugeicons/core-free-icons";
import { writeFileSync } from "node:fs";

const escape = (value) =>
  String(value)
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;");
const BOLD = new Set(["CheckIcon"]);
for (const [name, data] of Object.entries({
  AlertCircleIcon,
  ArrowDown01Icon,
  ArrowRight01Icon,
  ArrowRight02Icon,
  AudioWaveformIcon,
  BellIcon,
  BookOpen01Icon,
  Building03Icon,
  Camera01Icon,
  CancelCircleIcon,
  ChartLineData01Icon,
  CheckIcon,
  CheckmarkCircle02Icon,
  CloudOffIcon,
  CodeIcon,
  CopyIcon,
  CreditCardIcon,
  Cursor01Icon,
  Download01Icon,
  ElectricPlugsIcon,
  ExternalLinkIcon,
  Key01Icon,
  LoaderCircleIcon,
  LockPasswordIcon,
  Mic01Icon,
  MoreHorizontalIcon,
  PencilEdit01Icon,
  Plug01Icon,
  PlusIcon,
  RefreshIcon,
  Settings02Icon,
  SentIcon,
  ShieldCheckIcon,
  SparkleIcon,
  Sun01Icon,
  TrashIcon,
  TriangleAlertIcon,
  UserGroupIcon,
  UserIcon,
  Video01Icon,
  VolumeHighIcon,
  VolumeXIcon,
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
  // `weight="bold"` icons use a 2px stroke instead of the regular 1.5px.
  for (const [suffix, width] of BOLD.has(name)
    ? [
        ["", 1.5],
        ["Bold", 2],
      ]
    : [["", 1.5]]) {
    writeFileSync(
      new URL(`./icons/${name}${suffix}.svg`, import.meta.url),
      `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="${width}" stroke-linecap="round" stroke-linejoin="round">${elements}</svg>\n`,
    );
  }
}
