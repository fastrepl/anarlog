import {
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { cn } from "@anlg/utils";

import { Hint } from "./hint";

/**
 * Submenu whose description sits at the top of its content. Hovering the
 * trigger opens the submenu to the right, which is where a tooltip would go,
 * so the description lives inside instead.
 */
export function MenuGroup(props: {
  label: string;
  description: string;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <DropdownMenuSub>
      <DropdownMenuSubTrigger>{props.label}</DropdownMenuSubTrigger>
      <DropdownMenuSubContent className={cn(["w-56", props.className])}>
        <div className="text-muted-foreground px-2 pt-1.5 pb-1 text-xs leading-4">
          {props.description}
        </div>
        <DropdownMenuSeparator />
        {props.children}
      </DropdownMenuSubContent>
    </DropdownMenuSub>
  );
}

/** Shows an item's description beside the submenu while it is hovered. */
export function MenuHint(props: {
  description: string;
  children: React.ReactNode;
}) {
  return (
    <Hint side="right" content={props.description}>
      {props.children}
    </Hint>
  );
}
