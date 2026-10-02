import { useLocalSearchParams } from "expo-router";

import { NoteScreen } from "@/note/screen";

export default function NoteRoute() {
  const { id, listen } = useLocalSearchParams<{
    id: string;
    listen?: string;
  }>();
  return <NoteScreen key={id} id={id} autoListen={listen === "1"} />;
}
