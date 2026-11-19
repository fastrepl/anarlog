/// What the helper does with the real camera.
///
/// - Nobody uses Anarlog Camera: the real camera stays off, so its light does too, and the
///   extension shows its idle frame to anyone who opens it before the helper catches up.
/// - An app uses it while Anarlog isn't recording: the plain camera, so the person can leave
///   Anarlog Camera selected in every meeting app.
/// - An app uses it while Anarlog records: the camera with the Anarlog badge.
public struct FeedState: Equatable, Sendable {
  public var recording: Bool
  public var consumers: [String]

  public init(recording: Bool = false, consumers: [String] = []) {
    self.recording = recording
    self.consumers = consumers
  }

  public var capturesCamera: Bool { !consumers.isEmpty }
  public var showsBadge: Bool { recording && capturesCamera }
}
