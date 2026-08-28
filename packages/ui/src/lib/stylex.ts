import * as stylex from "@stylexjs/stylex";
import type { CSSProperties } from "react";

import { cn } from "@anlg/utils";

export type StyleXProps = {
  sx?: stylex.StyleXStyles;
};

export function mergeStyleXProps(
  sx: stylex.StyleXStyles,
  className?: string,
  style?: CSSProperties,
) {
  const resolved = stylex.props(sx);
  const mergedStyle =
    resolved.style || style ? { ...resolved.style, ...style } : undefined;

  return {
    ...resolved,
    className: cn([resolved.className, className]),
    style: mergedStyle,
  };
}
