import { NoteTab } from "@hypr/ui/components/ui/note-tab";

export function TabButton({
  label,
  active,
  onClick,
  trailing,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
  trailing?: React.ReactNode;
}) {
  return (
    <NoteTab isActive={active} onClick={onClick}>
      {label}
      {trailing}
    </NoteTab>
  );
}
