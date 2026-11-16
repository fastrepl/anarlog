import {
  ArrowLeft02Icon,
  ArrowRight02Icon,
  SidebarLeftIcon,
  PinIcon,
  UsersIcon,
  Folder01Icon,
  Calendar03Icon,
  ZapIcon,
  FileTextIcon,
} from "@hugeicons/core-free-icons";
import { writeFileSync } from "node:fs";

const escape = (value) =>
  String(value)
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;");
for (const [name, data] of Object.entries({
  ArrowLeft02Icon,
  ArrowRight02Icon,
  SidebarLeftIcon,
  PinIcon,
  UsersIcon,
  Folder01Icon,
  Calendar03Icon,
  ZapIcon,
  FileTextIcon,
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
