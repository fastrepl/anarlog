import Cocoa
import SwiftRs

private final class DictationPasteProvider: NSObject, NSPasteboardItemDataProvider {
  let text: String
  var didRead: (() -> Void)?
  init(text: String) { self.text = text }
  func pasteboard(
    _ pasteboard: NSPasteboard?, item: NSPasteboardItem,
    provideDataForType type: NSPasteboard.PasteboardType
  ) {
    item.setString(text, forType: type)
    // Restore after the target has requested and received its owned text representation.
    DispatchQueue.main.async {
      self.didRead?()
      self.didRead = nil
    }
  }
}

private final class DictationTarget {
  static let shared = DictationTarget()
  var element: AXUIElement?
  var token = ""
  var clipboardSnapshot: [NSPasteboardItem]?
  var clipboardChangeCount: Int?
  var clipboardProvider: DictationPasteProvider?

  func focusedElement() -> AXUIElement? {
    guard AXIsProcessTrusted() else { return nil }
    let system = AXUIElementCreateSystemWide()
    AXUIElementSetMessagingTimeout(system, 1)
    var value: CFTypeRef?
    guard
      AXUIElementCopyAttributeValue(system, kAXFocusedUIElementAttribute as CFString, &value)
        == .success,
      let value, CFGetTypeID(value) == AXUIElementGetTypeID()
    else { return nil }
    let element = unsafeBitCast(value, to: AXUIElement.self)
    AXUIElementSetMessagingTimeout(element, 1)
    return element
  }

  func acceptsText(_ element: AXUIElement) -> Bool {
    var role: CFTypeRef?
    var subrole: CFTypeRef?
    AXUIElementCopyAttributeValue(element, kAXRoleAttribute as CFString, &role)
    AXUIElementCopyAttributeValue(element, kAXSubroleAttribute as CFString, &subrole)
    if subrole as? String == kAXSecureTextFieldSubrole { return false }
    var editable = DarwinBoolean(false)
    AXUIElementIsAttributeSettable(element, kAXSelectedTextAttribute as CFString, &editable)
    if editable.boolValue { return true }
    AXUIElementIsAttributeSettable(element, kAXValueAttribute as CFString, &editable)
    return editable.boolValue
      && [kAXTextFieldRole, kAXTextAreaRole, kAXComboBoxRole].contains(role as? String ?? "")
  }

  func capture() -> String {
    element = nil
    guard let focused = focusedElement(), acceptsText(focused) else {
      return
        "Focus an editable text field and enable BlackMushi in System Settings > Privacy & Security > Accessibility. Password fields are excluded."
    }
    token = UUID().uuidString
    element = focused
    return "ok:" + token
  }

  func insert(token: String, text: String) -> String {
    guard token == self.token, let target = element, let focused = focusedElement(),
      CFEqual(target, focused), acceptsText(focused)
    else {
      return "The focused text field changed. Copy your last dictation from Settings > Dictation."
    }
    element = nil

    // AXSelectedText replaces the selection or inserts at the caret without touching the clipboard.
    if AXUIElementSetAttributeValue(target, kAXSelectedTextAttribute as CFString, text as CFString)
      == .success
    {
      return ""
    }
    guard let source = CGEventSource(stateID: .combinedSessionState),
      let down = CGEvent(keyboardEventSource: source, virtualKey: 9, keyDown: true),
      let up = CGEvent(keyboardEventSource: source, virtualKey: 9, keyDown: false)
    else { return "Could not insert text. Copy your last dictation from Settings > Dictation." }

    let pasteboard = NSPasteboard.general
    if clipboardChangeCount != pasteboard.changeCount {
      clipboardSnapshot = nil
    }
    var saved = clipboardSnapshot ?? []
    if clipboardSnapshot == nil {
      for original in pasteboard.pasteboardItems ?? [] {
        let copy = NSPasteboardItem()
        for type in original.types {
          guard let data = original.data(forType: type), copy.setData(data, forType: type) else {
            return
              "Could not preserve the clipboard. Copy your last dictation from Settings > Dictation."
          }
        }
        saved.append(copy)
      }
    }
    let item = NSPasteboardItem()
    let provider = DictationPasteProvider(text: text)
    item.setDataProvider(provider, forTypes: [.string])
    item.setString("", forType: NSPasteboard.PasteboardType("org.nspasteboard.ConcealedType"))
    item.setString("", forType: NSPasteboard.PasteboardType("org.nspasteboard.TransientType"))
    let clearedCount = pasteboard.clearContents()
    guard pasteboard.writeObjects([item]) else {
      clipboardSnapshot = saved
      clipboardChangeCount = clearedCount
      restoreClipboard(saved, expectedChangeCount: clearedCount, retries: 2)
      return "Could not prepare dictation for insertion."
    }
    let changeCount = pasteboard.changeCount
    clipboardSnapshot = saved
    clipboardChangeCount = changeCount
    clipboardProvider = provider
    provider.didRead = {
      self.restoreClipboard(saved, expectedChangeCount: changeCount, retries: 2)
    }
    // A destination may ignore Command-V and never request the promised text.
    DispatchQueue.main.asyncAfter(deadline: .now() + 30) {
      self.restoreClipboard(saved, expectedChangeCount: changeCount, retries: 2)
    }
    down.flags = .maskCommand
    up.flags = .maskCommand
    down.post(tap: .cghidEventTap)
    up.post(tap: .cghidEventTap)
    return ""
  }

  private func restoreClipboard(_ saved: [NSPasteboardItem], expectedChangeCount: Int, retries: Int)
  {
    guard clipboardChangeCount == expectedChangeCount else { return }
    let pasteboard = NSPasteboard.general
    if pasteboard.changeCount == expectedChangeCount {
      let count = pasteboard.clearContents()
      if !saved.isEmpty && !pasteboard.writeObjects(saved) {
        guard pasteboard.changeCount == count else {
          clipboardSnapshot = nil
          clipboardChangeCount = nil
          clipboardProvider = nil
          return
        }
        clipboardChangeCount = count
        if retries > 0 {
          DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
            self.restoreClipboard(saved, expectedChangeCount: count, retries: retries - 1)
          }
        } else {
          NSLog("Could not restore dictation clipboard; keeping its owned snapshot for recovery.")
        }
        return
      }
    }
    clipboardSnapshot = nil
    clipboardChangeCount = nil
    clipboardProvider = nil
  }

}

private func onMain<T>(_ work: () -> T) -> T {
  Thread.isMainThread ? work() : DispatchQueue.main.sync(execute: work)
}

@_cdecl("_capture_dictation_target")
public func captureDictationTarget() -> SRString {
  SRString(onMain { DictationTarget.shared.capture() })
}

@_cdecl("_insert_dictation_text")
public func insertDictationText(target: SRString, text: SRString) -> SRString {
  SRString(
    onMain { DictationTarget.shared.insert(token: target.toString(), text: text.toString()) })
}
