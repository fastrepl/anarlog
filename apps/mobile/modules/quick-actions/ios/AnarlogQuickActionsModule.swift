import AppIntents
import ExpoModulesCore
import Foundation
import UIKit

private let pendingActionKey = "quick-actions-pending-action"

@available(iOS 16.0, *)
struct ToggleListeningIntent: AppIntent {
  static var title: LocalizedStringResource = "Start or Stop Listening"
  static var description = IntentDescription(
    "Start a new meeting recording, or stop the recording already in progress."
  )
  static var openAppWhenRun: Bool = true

  func perform() async throws -> some IntentResult {
    QuickActionsService.shared.enqueue(action: "toggle_listening")
    return .result()
  }
}

@available(iOS 16.0, *)
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

public class AnarlogQuickActionsModule: Module {
  public func definition() -> ModuleDefinition {
    Name("AnarlogQuickActions")

    Events("onAction")

    OnCreate {
      QuickActionsService.shared.attach(module: self)
    }

    OnDestroy {
      QuickActionsService.shared.detach(module: self)
    }

    Function("consumePendingAction") {
      QuickActionsService.shared.consumePendingAction()
    }
  }

  fileprivate func emit(action: String) {
    sendEvent("onAction", ["action": action])
  }
}

public class AnarlogQuickActionsAppDelegateSubscriber:
  ExpoAppDelegateSubscriber
{
  public func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
  ) -> Bool {
    if #available(iOS 16.0, *) {
      AnarlogAppShortcuts.updateAppShortcutParameters()
    }
    return true
  }
}

private final class QuickActionsService {
  static let shared = QuickActionsService()

  private let defaults = UserDefaults.standard
  private let queue = DispatchQueue(label: "so.anarlog.quick-actions")
  private weak var module: AnarlogQuickActionsModule?

  private init() {}

  func attach(module: AnarlogQuickActionsModule) {
    self.module = module
  }

  func detach(module: AnarlogQuickActionsModule) {
    if self.module === module {
      self.module = nil
    }
  }

  func enqueue(action: String) {
    queue.sync {
      defaults.set(action, forKey: pendingActionKey)
    }
    DispatchQueue.main.async { [weak self] in
      self?.module?.emit(action: action)
    }
  }

  func consumePendingAction() -> String? {
    queue.sync {
      guard let action = defaults.string(forKey: pendingActionKey) else {
        return nil
      }
      defaults.removeObject(forKey: pendingActionKey)
      return action
    }
  }
}
