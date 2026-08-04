@main
enum LiveSessionIdentityTests {
  static func main() {
    testSupersededStartCannotActivate()
    testStaleTokenCannotStopNewerSession()
    testInvalidationPreventsPendingStartFromActivating()
  }

  private static func expect(_ condition: @autoclosure () -> Bool, _ message: String) {
    guard condition() else {
      fatalError(message)
    }
  }

  private static func testSupersededStartCannotActivate() {
    var identity = LiveSessionIdentity()
    let stale = identity.beginStart()
    let current = identity.beginStart()

    expect(
      !identity.activate(generation: stale.generation, token: stale.token),
      "superseded start activated"
    )
    expect(
      identity.activate(generation: current.generation, token: current.token),
      "current start did not activate"
    )
    expect(identity.matches(token: current.token), "current token is not active")
  }

  private static func testStaleTokenCannotStopNewerSession() {
    var identity = LiveSessionIdentity()
    let stale = identity.beginStart()
    expect(
      identity.activate(generation: stale.generation, token: stale.token),
      "initial start did not activate"
    )

    let current = identity.beginStart()
    expect(
      identity.activate(generation: current.generation, token: current.token),
      "newer start did not activate"
    )

    expect(!identity.deactivate(token: stale.token), "stale token stopped newer session")
    expect(identity.matches(token: current.token), "newer session was displaced")
    expect(identity.deactivate(token: current.token), "current token did not stop its session")
    expect(!identity.isActive, "session remained active after current stop")
  }

  private static func testInvalidationPreventsPendingStartFromActivating() {
    var identity = LiveSessionIdentity()
    let pending = identity.beginStart()

    identity.invalidate()

    expect(
      !identity.activate(generation: pending.generation, token: pending.token),
      "invalidated start activated"
    )
    expect(!identity.isActive, "invalidated session became active")
  }
}
