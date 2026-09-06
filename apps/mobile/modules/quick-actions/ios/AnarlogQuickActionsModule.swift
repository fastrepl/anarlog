import AppIntents
import ExpoModulesCore
import Foundation
import UIKit

private let pendingActionKey = "quick-actions-pending-action"

public class AnarlogQuickActionsModule: Module {
  public static func toggleListening() {
    QuickActionsService.shared.enqueue(action: "toggle_listening")
  }

  public func definition() -> ModuleDefinition {
    Name("AnarlogQuickActions")

    Events("onAction")

    View(AnarlogShortcutsButtonView.self) {}

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

class AnarlogShortcutsButtonView: ExpoView {
  private let button = ShortcutsUIButton(style: .automaticOutline)

  required init(appContext: AppContext? = nil) {
    super.init(appContext: appContext)
    addSubview(button)
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    button.frame = bounds
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
