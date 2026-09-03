import { type ClassValue, clsx } from "clsx";
import { extendTailwindMerge } from "tailwind-merge";

// `rounded-pill` is a custom radius token (`--radius-pill`); register it so it
// conflicts with the built-in `rounded-*` classes instead of stacking with them.
const twMerge = extendTailwindMerge({
  extend: { theme: { borderRadius: ["pill"] } },
});

/**
 * Combines multiple class names using clsx and merges Tailwind CSS classes intelligently.
 *
 * This utility function is essential for conditional className composition in React components,
 * especially when working with Tailwind CSS where class conflicts need to be resolved.
 *
 * @param inputs - Class values that can be strings, objects, arrays, or other types accepted by clsx
 * @returns A merged string of class names with Tailwind conflicts resolved
 *
 * @example
 * ```tsx
 * cn("px-2 py-1", "px-4") // => "py-1 px-4" (px-4 overrides px-2)
 * cn("text-red-500", condition && "text-blue-500") // Conditional classes
 * cn({ "bg-neutral-100": isActive, "bg-white": !isActive }) // Object syntax
 * ```
 */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
