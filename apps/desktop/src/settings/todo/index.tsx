import { ChevronDown } from "lucide-react";

import {
  Accordion,
  AccordionContent,
  AccordionHeader,
  AccordionItem,
  AccordionTriggerPrimitive,
} from "@hypr/ui/components/ui/accordion";
import { cn } from "@hypr/utils";

import { TodoProviderContent } from "./provider-content";
import { TODO_PROVIDERS } from "./shared";

export function SettingsTodo() {
  return (
    <div className="flex flex-col gap-4 pt-3">
      <Accordion type="multiple" className="flex flex-col">
        {TODO_PROVIDERS.map((provider) => (
          <AccordionItem
            key={provider.id}
            value={provider.id}
            className="group/provider border-b border-neutral-200"
          >
            <div className="grid grid-cols-[minmax(0,1fr)_auto] items-center">
              <AccordionHeader className="min-w-0">
                <AccordionTriggerPrimitive className="flex w-full min-w-0 items-center gap-2 py-3 text-left text-sm font-medium transition-all hover:no-underline">
                  {provider.icon}
                  <span>{provider.displayName}</span>
                </AccordionTriggerPrimitive>
              </AccordionHeader>
              <ChevronDown
                className={cn([
                  "size-4 shrink-0 text-neutral-400 transition-all duration-200",
                  "group-data-[state=open]/provider:rotate-180",
                ])}
              />
            </div>
            <AccordionContent className="pb-4">
              <div className="flex flex-col gap-3 pl-7">
                <TodoProviderContent config={provider} />
              </div>
            </AccordionContent>
          </AccordionItem>
        ))}
      </Accordion>
    </div>
  );
}
