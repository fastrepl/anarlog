"use client";

import * as AccordionPrimitive from "@radix-ui/react-accordion";
import { ChevronDown } from "lucide-react";

import { cn } from "@hypr/utils";

interface AccordionProps {
  title: string;
  icon?: string;
  children: React.ReactNode;
  defaultOpen?: boolean;
  className?: string;
}

export function Accordion({
  title,
  icon,
  children,
  defaultOpen = false,
  className,
}: AccordionProps) {
  return (
    <AccordionPrimitive.Root
      type="single"
      collapsible
      defaultValue={defaultOpen ? "item-1" : undefined}
      className={cn([
        "rounded-none border border-neutral-300 dark:border-neutral-700",
        className,
      ])}
    >
      <AccordionPrimitive.Item value="item-1">
        <AccordionPrimitive.Header>
          <AccordionPrimitive.Trigger className="group flex w-full items-center justify-between bg-neutral-50 px-4 py-3 text-left text-sm font-medium text-neutral-900 transition-colors hover:bg-neutral-100 dark:bg-neutral-900 dark:text-neutral-100 dark:hover:bg-neutral-800">
            <div className="flex items-center gap-2">
              {icon && (
                <span className="text-neutral-600 dark:text-neutral-400">
                  {icon}
                </span>
              )}
              <span>{title}</span>
            </div>
            <ChevronDown
              className="h-4 w-4 text-neutral-600 transition-transform duration-200 group-data-[state=open]:rotate-180 dark:text-neutral-400"
              aria-hidden
            />
          </AccordionPrimitive.Trigger>
        </AccordionPrimitive.Header>
        <AccordionPrimitive.Content className="data-[state=closed]:animate-accordion-up data-[state=open]:animate-accordion-down overflow-hidden">
          <div className="px-4 py-3 text-sm text-neutral-700 dark:text-neutral-300">
            {children}
          </div>
        </AccordionPrimitive.Content>
      </AccordionPrimitive.Item>
    </AccordionPrimitive.Root>
  );
}
