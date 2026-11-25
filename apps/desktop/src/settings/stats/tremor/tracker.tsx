// Tremor Tracker v1.0.0, Apache-2.0. See LICENSE.
// Source: tremorlabs/tremor@ca4d588f47820ff3d514d37fa4ee08a4222dec11.
// Adapted for calendar cells, Anarlog colors, and accessible data labels.
import type { HTMLAttributes } from "react";

import { cn } from "@anlg/utils";

type TrackerBlockProps = {
  key: string;
  color: string;
  label: string;
};

function Block({ color, label }: Omit<TrackerBlockProps, "key">) {
  return (
    <div
      role="listitem"
      aria-label={label}
      className="aspect-square size-full overflow-hidden"
    >
      <div className={cn(["size-full rounded-[3px]", color])} />
    </div>
  );
}

export function Tracker({
  data,
  className,
  ...props
}: HTMLAttributes<HTMLDivElement> & { data: TrackerBlockProps[] }) {
  return (
    <div
      className={cn([
        "group grid grid-flow-col grid-rows-7 items-center gap-[3px]",
        className,
      ])}
      role="list"
      data-tremor-component="tracker"
      {...props}
    >
      {data.map(({ key, ...block }) => (
        <Block key={key} {...block} />
      ))}
    </div>
  );
}
