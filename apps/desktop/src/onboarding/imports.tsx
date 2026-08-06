import { MeetingImportScreen } from "~/imports/screen";

export function ImportSection({ onContinue }: { onContinue: () => void }) {
  return <MeetingImportScreen compact onContinue={onContinue} />;
}
