import ApplicationServices
import Foundation

/// One reader per selected process. No global tree, permission prompt, or AX mutation.
/// Read it on a dedicated queue, never on audio callbacks or the audio writer queue.
public final class MeetingAXReader {
  private let application: AXUIElement
  private let bundleID: String
  private var previous: [(element: AXUIElement, label: String, id: String)] = []
  private var nextID = 0
  private let sessionID = UUID().uuidString

  public init(processID: Int32, bundleID: String) {
    application = AXUIElementCreateApplication(processID)
    self.bundleID = bundleID
  }

  public func snapshot() -> MeetingAXSnapshot {
    guard AXIsProcessTrusted() else {
      previous = []
      return .init(nodes: [], complete: false)
    }
    let deadline = ProcessInfo.processInfo.systemUptime + 0.4
    var complete = true
    var current: [(element: AXUIElement, label: String, id: String)] = []
    var nodes: [MeetingAXNode] = []
    var visited = Set<AXUIElement>()

    func read(_ element: AXUIElement, _ attribute: String) -> CFTypeRef? {
      guard ProcessInfo.processInfo.systemUptime < deadline else {
        complete = false
        return nil
      }
      AXUIElementSetMessagingTimeout(element, 0.04)
      var value: CFTypeRef?
      let error = AXUIElementCopyAttributeValue(element, attribute as CFString, &value)
      if error == .cannotComplete || error == .invalidUIElement || error == .apiDisabled {
        complete = false
      }
      return error == .success ? value : nil
    }
    func text(_ element: AXUIElement, _ attribute: String) -> String {
      let value = read(element, attribute)
      let string = (value as? String) ?? (value as? URL)?.absoluteString ?? ""
      // Do not turn truncated names into confident participant labels.
      guard string.utf8.count <= 512 else {
        complete = false
        return ""
      }
      return string
    }
    func walk(_ element: AXUIElement, parent: String?, depth: Int) {
      guard nodes.count < 1500, depth < 40,
        ProcessInfo.processInfo.systemUptime < deadline
      else {
        complete = false
        return
      }
      guard visited.insert(element).inserted else { return }
      let role = text(element, kAXRoleAttribute)
      let url = role == "AXWebArea" ? text(element, "AXURL") : ""
      // Do not descend into unrelated browser pages.
      if role == "AXWebArea" {
        let allowed =
          bundleID == "com.microsoft.teams2"
          ? MeetingAXAdapter.isTeamsPageURL(url) : MeetingAXAdapter.isMeetCallURL(url)
        if !allowed { return }
      }
      let title = text(element, kAXTitleAttribute)
      let description = text(element, kAXDescriptionAttribute)
      if title == "Message List" || description == "Message List" { return }
      let identifier = text(element, kAXIdentifierAttribute)
      // Webex's meeting chat history.
      if identifier == "MessagesView" { return }
      // Only static labels are read; never editable values, passwords, or chat input.
      let value = role == "AXStaticText" ? text(element, kAXValueAttribute) : ""
      let label = value.isEmpty ? title : value
      let identityLabel = label + "\n" + description + "\n" + url
      let id: String
      if let existing = previous.first(where: {
        CFEqual($0.element, element) && $0.label == identityLabel
      }) {
        id = existing.id
      } else {
        nextID += 1
        id = "\(sessionID)-\(nextID)"
      }
      current.append((element, identityLabel, id))
      nodes.append(
        .init(
          id: id, parentID: parent, role: role, title: label, description: description,
          identifier: identifier, url: url))
      // Editable controls cannot contain meeting controls or roster rows.
      if ["AXTextField", "AXTextArea", "AXSecureTextField"].contains(role) { return }
      guard let children = read(element, kAXChildrenAttribute) as? [AXUIElement] else { return }
      for child in children {
        walk(child, parent: id, depth: depth + 1)
        if !complete { break }
      }
    }
    // Windows only: exclude application menus and unrelated background processes.
    if var windows = read(application, kAXWindowsAttribute) as? [AXUIElement] {
      // An app on another Space can list no windows while its main window still answers.
      if windows.isEmpty {
        for attribute in [kAXMainWindowAttribute, kAXFocusedWindowAttribute] {
          if let window = read(application, attribute),
            CFGetTypeID(window) == AXUIElementGetTypeID()
          {
            windows.append(window as! AXUIElement)
          }
        }
      }
      for window in windows {
        // Do not traverse Zoom's Home/chat windows, or Webex's main window with its messages.
        let identifier = text(window, kAXIdentifierAttribute)
        let isCallWindow =
          switch bundleID {
          case "us.zoom.xos": identifier == "zm.meeting.window.main"
          case MeetingAXAdapter.webexBundleID: identifier == "PopOutWindow"
          default: true
          }
        if isCallWindow { walk(window, parent: nil, depth: 0) }
        if !complete { break }
      }
    } else {
      complete = false
    }
    previous = complete ? current : []
    return .init(nodes: nodes, complete: complete)
  }
}
