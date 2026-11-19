import CaptureCore
import Foundation

/// A bounded, in-memory snapshot. Raw UI strings are never journaled by the reader.
public struct MeetingAXNode {
  public let id: String
  public let parentID: String?
  public let role: String
  public let title: String
  public let description: String
  public let identifier: String
  public let url: String

  public init(
    id: String, parentID: String? = nil, role: String, title: String = "",
    description: String = "", identifier: String = "", url: String = ""
  ) {
    self.id = id
    self.parentID = parentID
    self.role = role
    self.title = title
    self.description = description
    self.identifier = identifier
    self.url = url
  }

  var labels: [String] { [title, description].filter { !$0.isEmpty } }
}

public struct MeetingAXSnapshot {
  public let nodes: [MeetingAXNode]
  public let complete: Bool

  public init(nodes: [MeetingAXNode], complete: Bool) {
    self.nodes = nodes
    self.complete = complete
  }
}

public enum MeetingAXResult: Equatable {
  case meeting(MeetingObservation)
  case unknown(String)
}

/// Whether this Mac's person is in a call in one app, as its windows show it. Anarlog reads it to
/// confirm the one call a recording scopes to.
public struct MeetingCallStatus: Equatable {
  /// The calls this app's windows can show: "zoom", "google-meet", "teams" or "webex".
  public let platform: String
  /// "in-call", "not-in-call", or "unknown" when the windows couldn't be read in full. Not in a
  /// call only covers what the windows show: a browser shows each window's front tab alone.
  public let status: String
  /// The call's own link when its window shows one (Meet in a browser).
  public let callURL: String?

  public init(platform: String, status: String, callURL: String? = nil) {
    self.platform = platform
    self.status = status
    self.callURL = callURL
  }
}

/// Platform policy consumes values, independent of AX IPC, recording, or the UI.
public enum MeetingAXAdapter {
  /// Browsers built on Chromium, whose Meet tab exposes the same accessibility tree as Chrome's.
  /// Only Chrome has been checked live; the others share its renderer.
  static let chromiumBrowsers: Set<String> = [
    "com.google.Chrome", "com.google.Chrome.beta", "com.google.Chrome.dev",
    "com.google.Chrome.canary", "org.chromium.Chromium", "com.brave.Browser",
    "com.microsoft.edgemac", "company.thebrowser.dia",
    "com.vivaldi.Vivaldi", "com.operasoftware.Opera",
  ]

  /// The Webex app; its meetings run in a native window of their own (`PopOutWindow`).
  static let webexBundleID = "Cisco-Systems.Spark"

  public static func supports(bundleID: String) -> Bool {
    ["us.zoom.xos", "com.microsoft.teams2", webexBundleID].contains(bundleID)
      || chromiumBrowsers.contains(bundleID)
  }

  static func isMeetCallURL(_ value: String) -> Bool {
    guard let url = URL(string: value), url.scheme == "https", url.host == "meet.google.com" else {
      return false
    }
    return url.path.range(
      of: #"^/(meet/)?[a-z]{3}-[a-z]{4}-[a-z]{3}/?$"#, options: .regularExpression) != nil
  }

  /// The pages Teams' app shows calls in: personal accounts on `teams.live.com`, work and school
  /// accounts on Microsoft's two work hosts. A call is still confirmed only by its call controls.
  static let teamsHosts: Set<String> = [
    "teams.live.com", "teams.microsoft.com", "teams.cloud.microsoft",
  ]

  static func isTeamsPageURL(_ value: String) -> Bool {
    guard let url = URL(string: value), url.scheme == "https", let host = url.host else {
      return false
    }
    return teamsHosts.contains(host)
  }

  /// The kind of call `project` can confirm in this app, or nil for an app it can't read.
  public static func platform(bundleID: String) -> String? {
    if bundleID == "us.zoom.xos" { return "zoom" }
    if bundleID == "com.microsoft.teams2" { return "teams" }
    if bundleID == webexBundleID { return "webex" }
    return chromiumBrowsers.contains(bundleID) ? "google-meet" : nil
  }

  /// `project`'s answer reduced to in a call or not. Only a complete read that found no call
  /// says "not-in-call"; anything else it can't confirm is "unknown".
  public static func callStatus(bundleID: String, snapshot: MeetingAXSnapshot)
    -> MeetingCallStatus?
  {
    guard let platform = platform(bundleID: bundleID) else { return nil }
    switch project(bundleID: bundleID, snapshot: snapshot) {
    case .meeting(let meeting):
      let url = snapshot.nodes.first { $0.id == meeting.scopeID }?.url ?? ""
      return .init(platform: platform, status: "in-call", callURL: url.isEmpty ? nil : url)
    case .unknown("no_confirmed_meeting"):
      return .init(platform: platform, status: "not-in-call")
    case .unknown:
      return .init(platform: platform, status: "unknown")
    }
  }

  /// The one app among these reads that shows a confirmed call, with that call's own link from
  /// the same read, or nil when none or several do. The recorder captures that app's call, so the
  /// link names the call Anarlog records; no later read can swap in another tab.
  public static func soleCall(_ reads: [(bundleID: String, snapshot: MeetingAXSnapshot)])
    -> (index: Int, callURL: String?)?
  {
    let calls = reads.indices.compactMap { index -> (index: Int, callURL: String?)? in
      let read = reads[index]
      guard let call = callStatus(bundleID: read.bundleID, snapshot: read.snapshot),
        call.status == "in-call"
      else { return nil }
      return (index, call.callURL)
    }
    return calls.count == 1 ? calls[0] : nil
  }

  public static func project(bundleID: String, snapshot: MeetingAXSnapshot) -> MeetingAXResult {
    guard snapshot.complete else { return .unknown("incomplete_snapshot") }
    let nodes = snapshot.nodes
    let byID = Dictionary(nodes.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })
    guard byID.count == nodes.count else { return .unknown("duplicate_node_identity") }
    func descendants(_ root: MeetingAXNode) -> [MeetingAXNode] {
      nodes.filter { candidate in
        var parent = candidate.parentID
        var seen = Set<String>()
        while let id = parent, seen.insert(id).inserted {
          if id == root.id { return true }
          parent = byID[id]?.parentID
        }
        return false
      }
    }

    if bundleID == "us.zoom.xos" {
      let windows = nodes.filter {
        $0.role == "AXWindow" && $0.identifier == "zm.meeting.window.main"
      }
      let calls = windows.filter { window in
        let controls = descendants(window).filter { $0.role == "AXButton" }
        return controls.contains { $0.identifier == "leave" }
          && controls.contains { $0.identifier == "audio" }
          && controls.contains { $0.identifier == "participant" }
      }
      guard calls.count == 1 else {
        return .unknown(calls.isEmpty ? "no_confirmed_meeting" : "multiple_meetings")
      }
      let window = calls[0]
      let content = descendants(window)
      let lists = content.filter {
        $0.role == "AXOutline" && $0.labels.contains("Participants list")
      }
      var roster: [MeetingParticipantObservation] = []
      if lists.count == 1 {
        for row in descendants(lists[0]).filter({ $0.role == "AXRow" }) {
          let names = descendants(row).filter { $0.role == "AXStaticText" }
          guard names.count == 1, let nameNode = names.first else { continue }
          let unique = Set(nameNode.labels)
          guard unique.count == 1, let label = unique.first, label.utf8.count <= 256 else {
            continue
          }
          let local = label.hasSuffix(" (Host, me)") || label.hasSuffix(" (me)")
          let name = label.replacingOccurrences(
            of: #" \((Host, me|me|Host|Guest)\)$"#, with: "", options: .regularExpression)
          guard !name.isEmpty else { continue }
          // IDs refer to AX element occurrences. Equal display names remain distinct.
          roster.append(.init(id: "\(window.id)/\(nameNode.id)", displayName: name, isLocal: local))
        }
      }
      return .meeting(
        .init(
          platform: "zoom", scopeID: window.id, participants: roster, speakers: [],
          speakerStatus: "unavailable", rosterVisible: lists.count == 1))
    }
    if chromiumBrowsers.contains(bundleID) {
      let calls = nodes.filter { area in
        guard area.role == "AXWebArea", isMeetCallURL(area.url) else { return false }
        let controls = descendants(area)
        return controls.contains { $0.role == "AXButton" && $0.labels.contains("Leave call") }
          && controls.contains { $0.labels.contains("Call controls") }
          && controls.contains {
            $0.role == "AXButton"
              && ($0.labels.contains("Turn on microphone")
                || $0.labels.contains("Turn off microphone"))
          }
      }
      guard calls.count == 1 else {
        return .unknown(calls.isEmpty ? "no_confirmed_meeting" : "multiple_meetings")
      }
      let area = calls[0]
      let lists = descendants(area).filter {
        $0.role == "AXList" && $0.labels.contains("Participants")
      }
      var roster: [MeetingParticipantObservation] = []
      if lists.count == 1 {
        for row in nodes.filter({ $0.parentID == lists[0].id && $0.role == "AXGroup" }) {
          let labels = Set(row.labels)
          guard labels.count == 1, let name = labels.first, name.utf8.count <= 256 else { continue }
          let names = descendants(row).filter {
            $0.role == "AXStaticText"
              && ($0.labels.contains(name) || $0.labels.contains(name + " (You)"))
          }
          guard names.count == 1 else { continue }
          roster.append(
            .init(
              id: "\(area.id)/\(names[0].id)", displayName: name,
              isLocal: names[0].labels.contains(name + " (You)")
                || descendants(row).contains {
                  $0.role == "AXStaticText" && $0.labels.contains("(You)")
                }))
        }
      }
      return .meeting(
        .init(
          platform: "google-meet", scopeID: area.id, participants: roster, speakers: [],
          speakerStatus: "unavailable", rosterVisible: lists.count == 1))
    }
    if bundleID == "com.microsoft.teams2" {
      let calls = nodes.filter { area in
        guard area.role == "AXWebArea", isTeamsPageURL(area.url) else {
          return false
        }
        let content = descendants(area)
        let toolbars = content.filter {
          $0.role == "AXToolbar" && $0.labels.contains("Meeting controls")
        }
        guard toolbars.count == 1 else { return false }
        let controls = descendants(toolbars[0])
        return controls.contains { $0.role == "AXButton" && $0.labels.contains("People") }
          && controls.contains {
            $0.role == "AXButton"
              && ($0.labels.contains("Unmute mic") || $0.labels.contains("Mute mic"))
          }
          && content.contains { $0.role == "AXButton" && $0.labels.contains("Leave") }
          && content.contains { $0.labels.contains("Calling controls") }
      }
      guard calls.count == 1 else {
        return .unknown(calls.isEmpty ? "no_confirmed_meeting" : "multiple_meetings")
      }
      let area = calls[0]
      let lists = descendants(area).filter {
        $0.role == "AXOutline" && $0.labels.contains("Attendees")
      }
      var roster: [MeetingParticipantObservation] = []
      if lists.count == 1 {
        for row in descendants(lists[0]).filter({ $0.role == "AXRow" }) {
          let names = descendants(row).filter { node in
            node.role == "AXStaticText"
              && node.labels.contains { name in
                !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                  && name.utf8.count <= 256 && row.labels.contains { $0.hasPrefix(name + ", ") }
              }
          }
          guard names.count == 1 else { continue }
          let labels = Set(
            names[0].labels.filter { name in row.labels.contains { $0.hasPrefix(name + ", ") } })
          guard labels.count == 1, let name = labels.first else { continue }
          // Organizer/muted status does not establish which participant is this Mac's user.
          roster.append(.init(id: "\(area.id)/\(names[0].id)", displayName: name, isLocal: nil))
        }
      }
      return .meeting(
        .init(
          platform: "teams", scopeID: area.id, participants: roster, speakers: [],
          speakerStatus: "unavailable", rosterVisible: lists.count == 1))
    }
    if bundleID == webexBundleID {
      // Webex's pre-join screen uses the same window; only a call shows End (or Leave) meeting.
      let calls = nodes.filter { window in
        guard window.role == "AXWindow", window.identifier == "PopOutWindow" else { return false }
        let controls = descendants(window).filter { $0.role == "AXButton" }
        return controls.contains { $0.identifier == "callControl_end" }
          && controls.contains { $0.identifier == "callControl_audio" }
      }
      guard calls.count == 1 else {
        return .unknown(calls.isEmpty ? "no_confirmed_meeting" : "multiple_meetings")
      }
      let window = calls[0]
      let lists = descendants(window).filter {
        $0.role == "AXOutline" && $0.labels.contains("Participants")
      }
      var roster: [MeetingParticipantObservation] = []
      var count: Int?
      if lists.count == 1 {
        // One group's header counts everyone, the person included: "..., 2 participant(s), ...".
        // A second group (a lobby) makes the count say less than the rows, so none is kept.
        let groups = descendants(lists[0]).filter {
          $0.identifier == "GenericParticipantListGroupCellView"
        }
        if groups.count == 1,
          let match = groups[0].description.firstMatch(of: #/, (\d{1,4}) participant\(s\),/#)
        {
          count = Int(match.1)
        }
        // A row's cell reads "<name>, Host, presenter, me, Muted, "; its first text is the name.
        let cells = descendants(lists[0]).filter {
          $0.identifier == "GenericParticipantListCellView"
        }
        for cell in cells {
          let names = descendants(cell).filter {
            $0.role == "AXStaticText" && !$0.title.isEmpty
              && cell.description.hasPrefix($0.title + ", ")
          }
          guard names.count == 1, names[0].title.utf8.count <= 256 else { continue }
          let name = names[0].title
          let roles = cell.description.dropFirst(name.count + 2).components(separatedBy: ", ")
          roster.append(
            .init(
              id: "\(window.id)/\(names[0].id)", displayName: name, isLocal: roles.contains("me")))
        }
      }
      return .meeting(
        .init(
          platform: "webex", scopeID: window.id, participants: roster, speakers: [],
          speakerStatus: "unavailable", rosterVisible: lists.count == 1, participantCount: count))
    }
    return .unknown("unsupported_application")
  }
}
