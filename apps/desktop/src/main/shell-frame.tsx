import { ClassicMainBody } from "./body";

import { MainShellBodyFrame, MainShellScaffold } from "~/shared/main";

export function ClassicMainShellFrame() {
  return (
    <MainShellScaffold>
      <MainShellBodyFrame autoSaveId="main-chat">
        <ClassicMainBody />
      </MainShellBodyFrame>
    </MainShellScaffold>
  );
}
