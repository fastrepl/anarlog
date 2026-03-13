import * as CheckboxPrimitive from "@radix-ui/react-checkbox";
import { Check } from "lucide-react";
import * as React from "react";

import { cn } from "@hypr/utils";

const Checkbox = React.forwardRef<
  React.ComponentRef<typeof CheckboxPrimitive.Root>,
  React.ComponentPropsWithoutRef<typeof CheckboxPrimitive.Root>
>(({ className, ...props }, ref) => (
  <CheckboxPrimitive.Root
    ref={ref}
    className={cn([
      "peer border-primary focus-visible:ring-ring data-[state=checked]:bg-primary data-[state=checked]:text-primary-foreground h-4 w-4 shrink-0 rounded-xs border shadow-xs focus-visible:ring-1 focus-visible:outline-hidden disabled:cursor-not-allowed disabled:opacity-50",
      className,
    ])}
    {...props}
  >
    <CheckboxPrimitive.Indicator
      className={cn(["flex items-center justify-center text-current"])}
    >
      <Check className="h-4 w-4" />
    </CheckboxPrimitive.Indicator>
  </CheckboxPrimitive.Root>
));
Checkbox.displayName = CheckboxPrimitive.Root.displayName;

export { Checkbox };
