// Tremor Tracker v1.0.0, Apache-2.0. See LICENSE.
// Source: tremorlabs/tremor@ca4d588f47820ff3d514d37fa4ee08a4222dec11.
// Adapted for calendar cells, Anarlog colors, and accessible data labels.
import * as HoverCard from "@radix-ui/react-hover-card";
import { type HTMLAttributes, useState } from "react";

import { useSquircleRef } from "@anlg/ui/hooks/use-squircle";
import { cn } from "@anlg/utils";

type TrackerBlockProps = {
  key: string;
  color: string;
  tooltip: string;
};

function Block({ color, tooltip }: Omit<TrackerBlockProps, "key">) {
  const [open, setOpen] = useState(false);

  return (
    <HoverCard.Root
      open={open}
      onOpenChange={setOpen}
      openDelay={0}
      closeDelay={0}
    >
      <HoverCard.Trigger onClick={() => setOpen(true)} asChild>
        <div
          role="listitem"
          aria-label={tooltip}
          className="aspect-square size-full overflow-hidden"
        >
          <div
            className={cn([
              "size-full rounded-[3px] transition-opacity hover:opacity-60",
              color,
            ])}
          />
        </div>
      </HoverCard.Trigger>
      <HoverCard.Portal>
        <Tooltip>{tooltip}</Tooltip>
      </HoverCard.Portal>
    </HoverCard.Root>
  );
}

function Tooltip({ children }: { children: string }) {
  const ref = useSquircleRef<HTMLDivElement>();
  return (
    <HoverCard.Content
      ref={ref}
      sideOffset={10}
      side="top"
      align="center"
      avoidCollisions
      className="bg-foreground text-background w-auto rounded-md px-2 py-1 text-xs shadow-md"
    >
      {children}
    </HoverCard.Content>
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
