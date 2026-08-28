import * as stylex from "@stylexjs/stylex";
import { Streamdown } from "streamdown";

import { cn } from "@anlg/utils";

import { changelogComponents } from "./components";

export interface ChangelogContentProps {
  content: string;
  components?: Record<string, React.ComponentType<any>>;
  className?: string;
  sx?: stylex.StyleXStyles;
}

export function ChangelogContent({
  content,
  components,
  className,
  sx,
}: ChangelogContentProps) {
  const merged = components
    ? { ...changelogComponents, ...components }
    : changelogComponents;
  const resolved = stylex.props(sx);

  return (
    <Streamdown
      {...resolved}
      className={cn([resolved.className, className])}
      components={merged}
      controls={false}
      allowedTags={{ banner: ["title", "variant"] }}
      isAnimating={false}
      linkSafety={{ enabled: false }}
    >
      {content}
    </Streamdown>
  );
}
