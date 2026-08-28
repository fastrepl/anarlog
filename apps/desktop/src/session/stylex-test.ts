import * as stylex from "@stylexjs/stylex";
import { expect } from "vitest";

export function getStyleClassNames(sx: stylex.StyleXStyles) {
  const classNames = new Set<string>();

  const collect = (value: unknown) => {
    if (!value) {
      return;
    }

    if (Array.isArray(value)) {
      value.forEach(collect);
      return;
    }

    if (typeof value !== "object") {
      return;
    }

    const compiledStyle = value as Record<string, unknown>;
    if (!compiledStyle.$$css) {
      return;
    }

    for (const [key, className] of Object.entries(compiledStyle)) {
      if (key !== "$$css" && typeof className === "string") {
        className
          .split(" ")
          .filter(Boolean)
          .forEach((name) => {
            classNames.add(name);
          });
      }
    }
  };

  collect(sx);
  return [...classNames];
}

export function hasStyle(
  element: Element | null | undefined,
  sx: stylex.StyleXStyles,
) {
  const classNames = getStyleClassNames(sx);
  return (
    classNames.length > 0 &&
    classNames.every((className) => element?.classList.contains(className))
  );
}

export function expectStyle(
  element: Element | null | undefined,
  sx: stylex.StyleXStyles,
) {
  expect(element).toBeTruthy();
  const missingClassNames = getStyleClassNames(sx).filter(
    (className) => !element?.classList.contains(className),
  );
  expect(
    missingClassNames,
    `Expected "${element?.className}" to include the StyleX classes`,
  ).toEqual([]);
}

export function expectNotStyle(
  element: Element | null | undefined,
  sx: stylex.StyleXStyles,
) {
  expect(element).toBeTruthy();
  expect(hasStyle(element, sx)).toBe(false);
}

export function closestWithStyle(element: Element, sx: stylex.StyleXStyles) {
  let current: Element | null = element;
  while (current) {
    if (hasStyle(current, sx)) {
      return current;
    }
    current = current.parentElement;
  }
  return null;
}
