import {
  ArrowLeft02Icon,
  ArrowRight02Icon,
  SidebarLeftIcon,
  ViewSidebarLeftIcon,
  PinIcon,
  UsersIcon,
  Folder01Icon,
  Calendar03Icon,
  Calendar04Icon,
  ZapIcon,
  FileTextIcon,
  FilterIcon,
  CheckIcon,
  ArrowRight01Icon,
  ArrowUp02Icon,
  ArrowDown02Icon,
  Sun01Icon,
  SortByUp01Icon,
  SortDescendingIcon,
  RefreshIcon,
  ArrowLeft01Icon,
  PlusIcon,
  MoreHorizontalIcon,
  CalendarOffIcon,
  MapPinIcon,
  Video01Icon,
  RepeatIcon,
} from "@hugeicons/core-free-icons";
import { writeFileSync } from "node:fs";

const escape = (value) =>
  String(value)
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;");
// Variants mirror the shipping props: `fill-current` fills the glyph, `weight="bold"` strokes at 2.
const variants = { FilterIcon: ["Filled"], ArrowUp02Icon: ["Bold"] };
for (const [name, data] of Object.entries({
  ArrowLeft02Icon,
  ArrowRight02Icon,
  SidebarLeftIcon,
  ViewSidebarLeftIcon,
  PinIcon,
  UsersIcon,
  Folder01Icon,
  Calendar03Icon,
  Calendar04Icon,
  ZapIcon,
  FileTextIcon,
  FilterIcon,
  CheckIcon,
  ArrowRight01Icon,
  ArrowUp02Icon,
  ArrowDown02Icon,
  Sun01Icon,
  SortByUp01Icon,
  SortDescendingIcon,
  RefreshIcon,
  ArrowLeft01Icon,
  PlusIcon,
  MoreHorizontalIcon,
  CalendarOffIcon,
  MapPinIcon,
  Video01Icon,
  RepeatIcon,
})) {
  for (const variant of ["", ...(variants[name] ?? [])]) {
    const strokeWidth = variant === "Bold" ? "2" : "1.5";
    const elements = data
      .map(
        ([tag, attrs]) =>
          `<${tag} ${Object.entries(attrs)
            .filter(([key]) => key !== "key")
            .map(([key, value]) => [
              key,
              key === "strokeWidth" ? strokeWidth : value,
            ])
            .map(
              ([key, value]) =>
                `${key.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)}="${escape(value)}"`,
            )
            .join(" ")} />`,
      )
      .join("");
    writeFileSync(
      new URL(`./icons/${name}${variant}.svg`, import.meta.url),
      `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="${variant === "Filled" ? "black" : "none"}" stroke="black" stroke-width="${strokeWidth}" stroke-linecap="round" stroke-linejoin="round">${elements}</svg>\n`,
    );
  }
}
