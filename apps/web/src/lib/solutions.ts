import { allSolutions } from "content-collections";

export type SolutionMenuItem = {
  to: string;
  label: string;
};

export const sortedSolutions = [...allSolutions].sort(
  (a, b) => a.order - b.order,
);

export const allSolutionMenuItems: SolutionMenuItem[] = sortedSolutions.map(
  (solution) => ({
    to: `/solution/${solution.slug}`,
    label: solution.label,
  }),
);

export const featuredSolutionMenuItems = allSolutionMenuItems.slice(0, 5);

export const allSolutionsMenuItem: SolutionMenuItem = {
  to: "/solutions/",
  label: "All Solutions",
};

export const showMoreSolutionsMenuItem: SolutionMenuItem = {
  to: "/solutions/",
  label: "Show more",
};
