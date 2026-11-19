import Cocoa
import SwiftRs

private final class DictationPasteProvider: NSObject, NSPasteboardItemDataProvider {
  let text: String
  /// Called once, when an app first asks for the text.
  var didRead: (() -> Void)?
  /// Set the moment any reader received the text.
  var provided = false
  init(text: String) { self.text = text }
  func pasteboard(
    _ pasteboard: NSPasteboard?, item: NSPasteboardItem,
    provideDataForType type: NSPasteboard.PasteboardType
  ) {
    provided = true
    item.setString(text, forType: type)
    let didRead = self.didRead
    self.didRead = nil
    didRead?()
  }
}

/// What a direct write over the selection did. Raw values cross the Rust bridge.
private enum DirectWrite: Int {
  /// Provably not written: nothing was sent, or the app answered with a refusal.
  case refused = 0
  case done = 1
  /// Sent, but the app never answered; it may still land.
  case unconfirmed = 2
}

private struct PasteRecord {
  let provider: DictationPasteProvider
  let saved: ClipboardSnapshot
  let changeCount: Int
}

private enum PasteFailure: Error {
  case clipboard
  case prepare
  case keystroke
}

private final class DictationTarget {
  static let shared = DictationTarget()
  /// How long a read paste may hold the clipboard when nobody reports it finished.
  static let readHold: TimeInterval = 1
  var element: AXUIElement?
  var token = ""
  var clipboardSnapshot: ClipboardSnapshot?
  var clipboardChangeCount: Int?
  var clipboardProvider: DictationPasteProvider?
  /// Recently captured fields, newest first, so a later edit can name the one it was for.
  var recent: [(token: String, element: AXUIElement)] = []
  var pastes: [Int: PasteRecord] = [:]
  var nextPaste = 1

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
        "Focus an editable text field and enable Anarlog in System Settings > Privacy & Security > Accessibility. Password fields are excluded."
    }
    token = UUID().uuidString
    element = focused
    recent.insert((token, focused), at: 0)
    if recent.count > 4 { recent.removeLast() }
    return "ok:" + token
  }

  func target(_ token: String) -> AXUIElement? {
    guard let element = recent.first(where: { $0.token == token })?.element else { return nil }
    AXUIElementSetMessagingTimeout(element, 1)
    return element
  }

  func release(_ token: String) {
    recent.removeAll { $0.token == token }
    if token == self.token { element = nil }
  }

  /// The captured field itself is still the focused text field: never whatever is focused now.
  func isFocused(_ token: String) -> Bool {
    guard let target = target(token), let focused = focusedElement() else { return false }
    return CFEqual(target, focused) && acceptsText(target)
  }

  private func string(_ element: AXUIElement, _ attribute: String) -> String? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, attribute as CFString, &value) == .success
    else { return nil }
    return value as? String
  }

  private func selectedRange(_ element: AXUIElement) -> CFRange? {
    var value: CFTypeRef?
    guard
      AXUIElementCopyAttributeValue(element, kAXSelectedTextRangeAttribute as CFString, &value)
        == .success, let value, CFGetTypeID(value) == AXValueGetTypeID()
    else { return nil }
    var range = CFRange()
    guard AXValueGetValue(value as! AXValue, .cfRange, &range) else { return nil }
    return range
  }

  /// The field around its selection, `window` UTF-16 units each side, as JSON; "" if unreadable.
  func read(_ token: String, window: Int) -> String {
    guard let element = target(token), let range = selectedRange(element) else { return "" }
    var units: [UInt16]?
    var length: Int?
    var count: CFTypeRef?
    if AXUIElementCopyAttributeValue(element, kAXNumberOfCharactersAttribute as CFString, &count)
      == .success, let count = count as? Int
    {
      length = count
    }
    if length.map({ $0 <= 200_000 }) ?? true, let value = string(element, kAXValueAttribute) {
      units = Array(value.utf16)
      length = units?.count
    }
    guard let length, range.location >= 0, range.length >= 0, range.length <= 100_000,
      range.location + range.length <= length
    else { return "" }
    let end = range.location + range.length
    var from = max(0, range.location - window)
    var to = min(length, end + window)
    if let units {
      // Never split a surrogate pair at the window's edges.
      if from > 0, from < range.location, UTF16.isTrailSurrogate(units[from]) { from += 1 }
      if to < units.count, to > end, UTF16.isLeadSurrogate(units[to - 1]) { to -= 1 }
    }
    func slice(_ start: Int, _ stop: Int) -> String? {
      if start >= stop { return "" }
      if let units { return String(decoding: units[start..<stop], as: UTF16.self) }
      var part = CFRange(location: start, length: stop - start)
      guard let value = AXValueCreate(.cfRange, &part) else { return nil }
      var text: CFTypeRef?
      guard
        AXUIElementCopyParameterizedAttributeValue(
          element, kAXStringForRangeParameterizedAttribute as CFString, value, &text) == .success
      else { return nil }
      return text as? String
    }
    guard let before = slice(from, range.location), let selected = slice(range.location, end),
      let after = slice(end, to)
    else { return "" }
    return json([
      "length": length, "start": range.location, "selected": selected, "before": before,
      "after": after,
    ])
  }

  func select(_ token: String, start: Int, length: Int) -> Bool {
    guard let element = target(token) else { return false }
    var range = CFRange(location: start, length: length)
    guard let value = AXValueCreate(.cfRange, &range) else { return false }
    return AXUIElementSetAttributeValue(element, kAXSelectedTextRangeAttribute as CFString, value)
      == .success
  }

  func write(_ token: String, text: String) -> DirectWrite {
    guard let element = target(token) else { return .refused }
    return setSelectedText(element, text)
  }

  /// Replaces the selection, or inserts at the caret, without touching the clipboard. Never
  /// sent where the selected text isn't settable: macOS 27 AppKit answers success there and
  /// changes nothing.
  private func setSelectedText(_ element: AXUIElement, _ text: String) -> DirectWrite {
    var settable = DarwinBoolean(false)
    guard
      AXUIElementIsAttributeSettable(element, kAXSelectedTextAttribute as CFString, &settable)
        == .success, settable.boolValue
    else { return .refused }
    switch AXUIElementSetAttributeValue(
      element, kAXSelectedTextAttribute as CFString, text as CFString)
    {
    case .success: return .done
    case .attributeUnsupported, .illegalArgument, .invalidUIElement, .notImplemented,
      .apiDisabled:
      return .refused
    // A timeout or failed messaging after the request went out: a busy app still applies it.
    default: return .unconfirmed
    }
  }

  /// Pastes into the captured field while it has focus; the paste's id, or -1 when no
  /// keystroke was sent.
  func paste(_ token: String, text: String) -> Int {
    guard isFocused(token), case .success(let id) = startPaste(text) else { return -1 }
    return id
  }

  /// 0 while nobody read the text, 1 once something did, 2 once it can no longer be read.
  func pasteState(_ id: Int) -> Int {
    guard let record = pastes[id] else { return 2 }
    return record.provider.provided ? 1 : 0
  }

  /// Withdraws a paste nobody read yet: its text can no longer reach any app. A paste read
  /// meanwhile stays on the clipboard until it is finished.
  func revokePaste(_ id: Int) -> Int {
    guard let record = pastes[id] else { return 2 }
    if record.provider.provided { return 1 }
    pastes[id] = nil
    restoreClipboard(record.saved, expectedChangeCount: record.changeCount, retries: 2)
    let pasteboard = NSPasteboard.general
    if pasteboard.changeCount == record.changeCount {
      // The restore could not run; withdraw the promise itself.
      pasteboard.clearContents()
    }
    return 2
  }

  /// The app is done with a paste it read: the person's clipboard goes back, unless they
  /// changed it since.
  func finishPaste(_ id: Int) {
    guard let record = pastes[id], record.provider.provided else { return }
    pastes[id] = nil
    restoreClipboard(record.saved, expectedChangeCount: record.changeCount, retries: 2)
  }

  /// The app and window the captured field is in, as JSON; "" once it is gone.
  func facts(_ token: String) -> String {
    guard let element = target(token) else { return "" }
    var pid: pid_t = 0
    guard AXUIElementGetPid(element, &pid) == .success, pid > 0 else { return "" }
    let app = NSRunningApplication(processIdentifier: pid)
    var title: String?
    var window: CFTypeRef?
    if AXUIElementCopyAttributeValue(element, kAXWindowAttribute as CFString, &window) == .success,
      let window, CFGetTypeID(window) == AXUIElementGetTypeID()
    {
      title = string(window as! AXUIElement, kAXTitleAttribute)
    }
    var facts: [String: Any] = [
      "secure": string(element, kAXSubroleAttribute) == kAXSecureTextFieldSubrole
    ]
    facts["bundleId"] = app?.bundleIdentifier
    facts["appName"] = app?.localizedName
    facts["title"] = title
    return json(facts)
  }

  /// Static text of the captured field's window in reading order, the newest `max` characters,
  /// within a small time and node budget. Secure fields and the field itself are skipped.
  func windowText(_ token: String, max: Int) -> String {
    guard let element = target(token) else { return "" }
    var window: CFTypeRef?
    guard
      AXUIElementCopyAttributeValue(element, kAXWindowAttribute as CFString, &window) == .success,
      let window, CFGetTypeID(window) == AXUIElementGetTypeID()
    else { return "" }
    let deadline = Date().addingTimeInterval(0.25)
    var parts: [String] = []
    var visited = 0
    func walk(_ node: AXUIElement, depth: Int) {
      guard visited < 800, depth < 30, Date() < deadline, !CFEqual(node, element) else { return }
      visited += 1
      AXUIElementSetMessagingTimeout(node, 0.2)
      if string(node, kAXSubroleAttribute) == kAXSecureTextFieldSubrole { return }
      if string(node, kAXRoleAttribute) == kAXStaticTextRole,
        let text = string(node, kAXValueAttribute)?.trimmingCharacters(in: .whitespacesAndNewlines),
        !text.isEmpty
      {
        parts.append(text)
      }
      var children: CFTypeRef?
      guard
        AXUIElementCopyAttributeValue(node, kAXChildrenAttribute as CFString, &children)
          == .success, let children = children as? [AXUIElement]
      else { return }
      for child in children { walk(child, depth: depth + 1) }
    }
    walk(window as! AXUIElement, depth: 0)
    return String(parts.joined(separator: "\n").suffix(max))
  }

  private func json(_ object: [String: Any]) -> String {
    guard let data = try? JSONSerialization.data(withJSONObject: object) else { return "" }
    return String(decoding: data, as: UTF8.self)
  }

  func insert(token: String, text: String) -> String {
    guard token == self.token, let target = element, let focused = focusedElement(),
      CFEqual(target, focused), acceptsText(focused)
    else {
      return "The focused text field changed. Copy your last dictation from Settings > Dictation."
    }
    element = nil

    switch setSelectedText(target, text) {
    case .done: return ""
    // Pasting too could type the text twice.
    case .unconfirmed:
      return
        "The app did not confirm the dictation. If it is missing, copy it from Settings > Dictation."
    case .refused: break
    }
    switch startPaste(text) {
    case .success: return ""
    case .failure(.keystroke):
      return "Could not insert text. Copy your last dictation from Settings > Dictation."
    case .failure(.clipboard):
      return
        "Could not preserve the clipboard. Copy your last dictation from Settings > Dictation."
    case .failure(.prepare): return "Could not prepare dictation for insertion."
    }
  }

  /// Puts `text` on the clipboard as a promise, sends Command-V, and restores the person's
  /// clipboard once the read paste is finished (or after 30 s unread), unless they changed it
  /// meanwhile.
  private func startPaste(_ text: String) -> Result<Int, PasteFailure> {
    guard let source = CGEventSource(stateID: .combinedSessionState),
      let down = CGEvent(keyboardEventSource: source, virtualKey: 9, keyDown: true),
      let up = CGEvent(keyboardEventSource: source, virtualKey: 9, keyDown: false)
    else { return .failure(.keystroke) }

    let pasteboard = NSPasteboard.general
    if clipboardChangeCount != pasteboard.changeCount {
      clipboardSnapshot = nil
    }
    guard let saved = clipboardSnapshot ?? ClipboardSnapshot(pasteboard) else {
      return .failure(.clipboard)
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
      return .failure(.prepare)
    }
    let changeCount = pasteboard.changeCount
    clipboardSnapshot = saved
    clipboardChangeCount = changeCount
    clipboardProvider = provider
    let id = nextPaste
    nextPaste += 1
    pastes[id] = PasteRecord(provider: provider, saved: saved, changeCount: changeCount)
    pastes = pastes.filter { $0.key > id - 4 }
    // An app keeps reading after its first request; restoring then moves the changeCount
    // under it and it pastes nothing. The text stays until finishPaste, at most `readHold`.
    provider.didRead = {
      DispatchQueue.main.asyncAfter(deadline: .now() + Self.readHold) { self.finishPaste(id) }
    }
    // A destination may ignore Command-V and never request the promised text.
    DispatchQueue.main.asyncAfter(deadline: .now() + 30) {
      self.restoreClipboard(saved, expectedChangeCount: changeCount, retries: 2)
    }
    down.flags = .maskCommand
    up.flags = .maskCommand
    down.post(tap: .cghidEventTap)
    up.post(tap: .cghidEventTap)
    return .success(id)
  }

  private func restoreClipboard(_ saved: ClipboardSnapshot, expectedChangeCount: Int, retries: Int)
  {
    guard clipboardChangeCount == expectedChangeCount else { return }
    let pasteboard = NSPasteboard.general
    if pasteboard.changeCount == expectedChangeCount {
      let count = pasteboard.clearContents()
      if !saved.write(to: pasteboard) {
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

@_cdecl("_dictation_target_focused")
public func dictationTargetFocused(target: SRString) -> Bool {
  onMain { DictationTarget.shared.isFocused(target.toString()) }
}

@_cdecl("_dictation_target_read")
public func dictationTargetRead(target: SRString, window: Int) -> SRString {
  SRString(onMain { DictationTarget.shared.read(target.toString(), window: window) })
}

@_cdecl("_dictation_target_select")
public func dictationTargetSelect(target: SRString, start: Int, length: Int) -> Bool {
  onMain { DictationTarget.shared.select(target.toString(), start: start, length: length) }
}

@_cdecl("_dictation_target_write")
public func dictationTargetWrite(target: SRString, text: SRString) -> Int {
  onMain { DictationTarget.shared.write(target.toString(), text: text.toString()).rawValue }
}

@_cdecl("_dictation_target_paste")
public func dictationTargetPaste(target: SRString, text: SRString) -> Int {
  onMain { DictationTarget.shared.paste(target.toString(), text: text.toString()) }
}

@_cdecl("_dictation_paste_state")
public func dictationPasteState(id: Int) -> Int {
  onMain { DictationTarget.shared.pasteState(id) }
}

@_cdecl("_dictation_paste_revoke")
public func dictationPasteRevoke(id: Int) -> Int {
  onMain { DictationTarget.shared.revokePaste(id) }
}

@_cdecl("_dictation_paste_finish")
public func dictationPasteFinish(id: Int) -> Bool {
  onMain { DictationTarget.shared.finishPaste(id) }
  return true
}

@_cdecl("_dictation_target_facts")
public func dictationTargetFacts(target: SRString) -> SRString {
  SRString(onMain { DictationTarget.shared.facts(target.toString()) })
}

@_cdecl("_dictation_target_window_text")
public func dictationTargetWindowText(target: SRString, max: Int) -> SRString {
  SRString(onMain { DictationTarget.shared.windowText(target.toString(), max: max) })
}

@_cdecl("_dictation_target_release")
public func dictationTargetRelease(target: SRString) -> Bool {
  onMain { DictationTarget.shared.release(target.toString()) }
  return true
}
