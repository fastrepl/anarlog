struct LiveSessionIdentity {
  private var generation: UInt64 = 0
  private var activeToken: String?

  var isActive: Bool {
    activeToken != nil
  }

  mutating func beginStart() -> (generation: UInt64, token: String) {
    generation &+= 1
    activeToken = nil
    return (generation, String(generation))
  }

  func isCurrent(generation: UInt64) -> Bool {
    self.generation == generation
  }

  mutating func activate(generation: UInt64, token: String) -> Bool {
    guard isCurrent(generation: generation) else {
      return false
    }

    activeToken = token
    return true
  }

  func matches(token: String) -> Bool {
    activeToken == token
  }

  mutating func deactivate(token: String) -> Bool {
    guard matches(token: token) else {
      return false
    }

    activeToken = nil
    return true
  }

  mutating func invalidate() {
    generation &+= 1
    activeToken = nil
  }
}
