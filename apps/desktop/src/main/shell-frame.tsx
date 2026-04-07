import { ClassicMainBody } from "./body";
import { ClassicMainSidebar } from "./shell-sidebar";

import { MainShellBodyFrame, MainShellScaffold } from "~/shared/main";

export function ClassicMainShellFrame() {
  return (
    <MainShellScaffold>
      <ClassicMainSidebar />
      <MainShellBodyFrame autoSaveId="main-chat">
        <ClassicMainBody />
      </MainShellBodyFrame>
    </MainShellScaffold>
  );
}
