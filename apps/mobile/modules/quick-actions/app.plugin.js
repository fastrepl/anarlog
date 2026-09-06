const { copyFileSync } = require("node:fs");
const path = require("node:path");
const { IOSConfig, withXcodeProject } = require("expo/config-plugins");

module.exports = function withAnarlogAppShortcuts(config) {
  return withXcodeProject(config, (config) => {
    const projectName = config.modRequest.projectName;
    const filename = "AnarlogAppShortcuts.swift";
    const filepath = path.join(projectName, filename);

    // AppShortcutsProvider must be compiled in the app target to be indexed by iOS.
    copyFileSync(
      path.join(__dirname, "app-target", filename),
      path.join(config.modRequest.platformProjectRoot, filepath),
    );
    IOSConfig.XcodeUtils.addBuildSourceFileToGroup({
      filepath,
      groupName: projectName,
      project: config.modResults,
    });
    return config;
  });
};
