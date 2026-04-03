import { Body } from "./body";
import { MainShellBodyFrame } from "./body-frame";
import { MainShellScaffold } from "./shell-scaffold";
import { MainShellSidebar } from "./shell-sidebar";

export function MainShellFrame() {
  return (
    <MainShellScaffold>
      <MainShellSidebar />
      <MainShellBodyFrame autoSaveId="main-chat">
        <Body />
      </MainShellBodyFrame>
    </MainShellScaffold>
  );
}
