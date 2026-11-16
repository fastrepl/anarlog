// Exports the sharing Hugeicons (the same icons shipping's share surfaces use) as SVGs.
// node apps/desktop-gpui/src/product/sharing/export-icons.mjs
import {
  AddTeamIcon,
  AlertCircleIcon,
  ArrowDown01Icon,
  ArrowLeft01Icon,
  Building03Icon,
  CheckIcon,
  CopyIcon,
  ExternalLinkIcon,
  FileTextIcon,
  Folder01Icon,
  GlobeIcon,
  LinkIcon,
  LoaderCircleIcon,
  LockKeyIcon,
  LockPasswordIcon,
  Login03Icon,
  Mail02Icon,
  PaperclipIcon,
  RefreshIcon,
  Share08Icon,
  TrashIcon,
  TriangleAlertIcon,
  UserPlusIcon,
  UsersIcon,
  XIcon,
  ZapIcon,
} from "@hugeicons/core-free-icons";
import { writeFileSync } from "node:fs";

const escape = (value) =>
  String(value)
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;");
for (const [name, data] of Object.entries({
  AddTeamIcon,
  AlertCircleIcon,
  ArrowDown01Icon,
  ArrowLeft01Icon,
  Building03Icon,
  CheckIcon,
  CopyIcon,
  ExternalLinkIcon,
  FileTextIcon,
  Folder01Icon,
  GlobeIcon,
  LinkIcon,
  LoaderCircleIcon,
  LockKeyIcon,
  LockPasswordIcon,
  Login03Icon,
  Mail02Icon,
  PaperclipIcon,
  RefreshIcon,
  Share08Icon,
  TrashIcon,
  TriangleAlertIcon,
  UserPlusIcon,
  UsersIcon,
  XIcon,
  ZapIcon,
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
