import { cn } from "@hypr/utils";

const HEADING_SHARED = "text-gray-700 font-semibold text-sm mb-1 min-h-6";
const HEADING_WITH_MARGIN = "mt-4 first:mt-0";

export const streamdownComponents = {
  h1: (props: React.HTMLAttributes<HTMLHeadingElement>) => (
    <h1 className={cn([HEADING_SHARED, HEADING_WITH_MARGIN, "text-xl"])}>
      {props.children as React.ReactNode}
    </h1>
  ),
  h2: (props: React.HTMLAttributes<HTMLHeadingElement>) => (
    <h2 className={cn([HEADING_SHARED, HEADING_WITH_MARGIN, "text-lg"])}>
      {props.children as React.ReactNode}
    </h2>
  ),
  h3: (props: React.HTMLAttributes<HTMLHeadingElement>) => (
    <h3 className={cn([HEADING_SHARED, HEADING_WITH_MARGIN, "text-base"])}>
      {props.children as React.ReactNode}
    </h3>
  ),
  h4: (props: React.HTMLAttributes<HTMLHeadingElement>) => (
    <h4 className={cn([HEADING_SHARED, HEADING_WITH_MARGIN, "text-sm"])}>
      {props.children as React.ReactNode}
    </h4>
  ),
  h5: (props: React.HTMLAttributes<HTMLHeadingElement>) => (
    <h5 className={cn([HEADING_SHARED, HEADING_WITH_MARGIN, "text-sm"])}>
      {props.children as React.ReactNode}
    </h5>
  ),
  h6: (props: React.HTMLAttributes<HTMLHeadingElement>) => (
    <h6 className={cn([HEADING_SHARED, HEADING_WITH_MARGIN, "text-xs"])}>
      {props.children as React.ReactNode}
    </h6>
  ),
  ul: (props: React.HTMLAttributes<HTMLUListElement>) => (
    <ul className="relative mb-1 block list-disc pl-6">
      {props.children as React.ReactNode}
    </ul>
  ),
  ol: (props: React.HTMLAttributes<HTMLOListElement>) => (
    <ol className="relative mb-1 block list-decimal pl-6">
      {props.children as React.ReactNode}
    </ol>
  ),
  li: (props: React.HTMLAttributes<HTMLLIElement>) => (
    <li className="mb-1">{props.children as React.ReactNode}</li>
  ),
  p: (props: React.HTMLAttributes<HTMLParagraphElement>) => (
    <p className="mb-1">{props.children as React.ReactNode}</p>
  ),
} as const;
