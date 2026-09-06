internal import AnarlogQuickActions
import AppIntents

struct ToggleListeningIntent: AppIntent {
  static var title: LocalizedStringResource = "Start or Stop Listening"
  static var description = IntentDescription(
    "Start a new meeting recording, or stop the recording already in progress."
  )
  static var openAppWhenRun: Bool = true

  func perform() async throws -> some IntentResult {
    AnarlogQuickActionsModule.toggleListening()
    return .result()
  }
}

struct AnarlogAppShortcuts: AppShortcutsProvider {
  static var appShortcuts: [AppShortcut] {
    AppShortcut(
      intent: ToggleListeningIntent(),
      phrases: [
        "Start listening with \(.applicationName)",
        "Stop listening with \(.applicationName)",
        "Quick capture with \(.applicationName)",
      ],
      shortTitle: "Start Listening",
      systemImageName: "waveform"
    )
  }

  static var shortcutTileColor: ShortcutTileColor = .yellow
}
