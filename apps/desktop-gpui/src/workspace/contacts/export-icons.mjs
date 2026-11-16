// Writes the contacts Hugeicons that `workspace/`, `chat/` and the shared assets do not embed.
// Run with Node from a checkout whose `node_modules` has `@hugeicons/core-free-icons`.
import {
  ArrowTurnBackwardIcon,
  ArrowUpDownIcon,
  Building03Icon,
  Camera01Icon,
  CircleMinusIcon,
  PinIcon,
} from "@hugeicons/core-free-icons";
import { writeFileSync } from "node:fs";

const escape = (value) =>
  String(value)
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;");
// `weight="bold"` strokes at 2, as shipping's pinned contacts do.
for (const [name, data, strokeWidth] of [
  ["ArrowTurnBackwardIcon", ArrowTurnBackwardIcon, "1.5"],
  ["ArrowUpDownIcon", ArrowUpDownIcon, "1.5"],
  ["Building03Icon", Building03Icon, "1.5"],
  ["Camera01Icon", Camera01Icon, "1.5"],
  ["CircleMinusIcon", CircleMinusIcon, "1.5"],
  ["PinIconBold", PinIcon, "2"],
]) {
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
    new URL(`./icons/${name}.svg`, import.meta.url),
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="${strokeWidth}" stroke-linecap="round" stroke-linejoin="round">${elements}</svg>\n`,
  );
}
