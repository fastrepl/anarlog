import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@hypr/ui/components/ui/command";

export type OpenNoteDialogItem = {
  id: string;
  title: string;
  subtitle: string;
};

export function OpenNoteDialogView({
  open,
  items,
  onOpenChange,
  onSelect,
}: {
  open: boolean;
  items: OpenNoteDialogItem[];
  onOpenChange: (open: boolean) => void;
  onSelect: (id: string) => void;
}) {
  return (
    <CommandDialog open={open} onOpenChange={onOpenChange}>
      <CommandInput placeholder="Open a session..." />
      <CommandList>
        <CommandEmpty>No sessions found.</CommandEmpty>
        <CommandGroup heading="Sessions">
          {items.map((item) => (
            <CommandItem
              key={item.id}
              value={`${item.title} ${item.subtitle}`}
              onSelect={() => onSelect(item.id)}
            >
              <div className="flex min-w-0 flex-1 flex-col">
                <span className="truncate">{item.title}</span>
                <span className="truncate text-xs text-neutral-500">
                  {item.subtitle}
                </span>
              </div>
            </CommandItem>
          ))}
        </CommandGroup>
      </CommandList>
    </CommandDialog>
  );
}
