import { allDocs } from "content-collections";

export const docsStructure = {
  sections: [
    "about",
    "getting started",
    "guides",
    "calendar",
    "cli",
    "developers",
    "pro",
    "faq",
  ],
  defaultPages: {
    about: "about/hello-world",
    "getting-started": "getting-started/installation",
    guides: "guides/data",
    calendar: "calendar/apple",
    cli: "cli",
    developers: "developers/analytics",
    pro: "pro/activation",
    faq: "faq/general",
  } as Record<string, string>,
};

export function getDocsBySection() {
  const sectionGroups: Record<
    string,
    { title: string; docs: (typeof allDocs)[0][] }
  > = {};

  allDocs.forEach((doc) => {
    if (doc.slug === "index" || doc.isIndex) {
      return;
    }

    const sectionName = doc.section;

    if (!sectionGroups[sectionName]) {
      sectionGroups[sectionName] = {
        title: sectionName,
        docs: [],
      };
    }

    sectionGroups[sectionName].docs.push(doc);
  });

  Object.keys(sectionGroups).forEach((sectionName) => {
    sectionGroups[sectionName].docs.sort((a, b) => a.order - b.order);
  });

  const sections = docsStructure.sections
    .map((sectionId) => {
      return Object.values(sectionGroups).find(
        (group) => group.title.toLowerCase() === sectionId.toLowerCase(),
      );
    })
    .filter(
      (section): section is NonNullable<typeof section> =>
        section !== undefined,
    );

  return { sections };
}
