import { commands as detectCommands } from "@anlg/plugin-detect";

export function pauseCompetingApplicationTermination() {
  const pause = detectCommands.setCompetingApplicationTerminationPaused(true);

  return () => {
    void pause.then(() =>
      detectCommands.setCompetingApplicationTerminationPaused(false),
    );
  };
}
