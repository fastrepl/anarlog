import { Streamdown } from "streamdown";

import { changelogComponents } from "./components";

export interface ChangelogContentProps {
  content: string;
  components?: Record<string, React.ComponentType<any>>;
  className?: string;
}

export function ChangelogContent({
  content,
  components,
  className,
}: ChangelogContentProps) {
  const merged = components
    ? { ...changelogComponents, ...components }
    : changelogComponents;

  return (
    <Streamdown
      className={className}
      components={merged}
      allowedTags={{ banner: ["title", "variant"] }}
      isAnimating={false}
      linkSafety={{ enabled: false }}
    >
      {content}
    </Streamdown>
  );
}
