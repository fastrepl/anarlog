import { allLegals } from "content-collections";

const LEGAL_ORDER = ["terms", "privacy", "cookies", "dpa"];

export function getOrderedLegals() {
  return [...allLegals].sort((a, b) => {
    const aIndex = LEGAL_ORDER.indexOf(a.slug);
    const bIndex = LEGAL_ORDER.indexOf(b.slug);

    if (aIndex === -1 && bIndex === -1) {
      return a.title.localeCompare(b.title);
    }

    if (aIndex === -1) return 1;
    if (bIndex === -1) return -1;

    return aIndex - bIndex;
  });
}

export function getLegalSections() {
  return [
    {
      title: "Legal",
      docs: getOrderedLegals(),
    },
  ];
}
