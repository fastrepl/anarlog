import Foundation

/// How a style moves. Every moving part is a pure function of a time in seconds, so the live
/// camera (frame timestamps) and renders draw the same thing, and time 0 is the still frame. Everything repeats every `loop` seconds and
/// changes slowly and softly: nothing flashes, and nothing pulls the eye in someone else's tile.
public enum WatermarkMotion {
  /// One loop of every style, about 4.67 s (112 frames at 24 fps). Every period below divides it
  /// exactly.
  public static let loop: Double = 112.0 / 24.0
  /// One breath of the badge's and banner's red dot: three per loop, about 1.56 s each.
  public static let dotPeriod: Double = loop / 3
  /// One breath of the edge glow and of the person's own image: two per loop, about 2.33 s each.
  public static let glowPeriod: Double = loop / 2
  /// How faint the dot gets at the bottom of a breath.
  public static let dotMinAlpha: Double = 0.35
  /// How faint the glow gets at the bottom of a breath.
  public static let glowMinStrength: Double = 0.55
  /// How faint the person's own image gets in the mark layout.
  public static let imageMinAlpha: Double = 0.75

  /// The dot's alpha: full at time 0, easing down to `dotMinAlpha` and back up.
  public static func dotAlpha(at time: Double) -> Double {
    breath(time, period: dotPeriod, low: dotMinAlpha)
  }

  /// The edge glow's strength: full at time 0, easing down to `glowMinStrength` and back up.
  public static func glowStrength(at time: Double) -> Double {
    breath(time, period: glowPeriod, low: glowMinStrength)
  }

  /// The person's own image in the mark layout, breathing like the glow.
  public static func imageAlpha(at time: Double) -> Double {
    breath(time, period: glowPeriod, low: imageMinAlpha)
  }

  /// How far toward the accent colour the mark is at `u` (0 its left edge, 1 its right) at
  /// `time`: 0 is the mark's own colour. A soft accent front sweeps across it left to right, it
  /// holds, a front of its own colour follows, and it holds again, once per loop. At time 0 it is
  /// all its own colour.
  public static func markAccent(at time: Double, along u: Double) -> Double {
    let p = phase(time, period: loop)
    return front(u, progress: (p - 0.05) / 0.4) - front(u, progress: (p - 0.55) / 0.4)
  }

  /// Where `time` falls in a loop of `period`, in 0..<1, for any time including negative ones.
  static func phase(_ time: Double, period: Double) -> Double {
    guard time.isFinite else { return 0 }
    let p = time.truncatingRemainder(dividingBy: period) / period
    return p < 0 ? p + 1 : p
  }

  /// 1 at the start of each period, easing (cosine) to `low` halfway and back.
  private static func breath(_ time: Double, period: Double, low: Double) -> Double {
    let wave = (1 + cos(2 * .pi * phase(time, period: period))) / 2
    return low + (1 - low) * wave
  }

  /// How much of the mark at `u` a soft front has passed, as the front crosses at a steady pace
  /// from just before the left edge (progress 0) to just past the right edge (progress 1). The
  /// front is most of the mark wide, so the mark shows a gradient while it passes.
  private static func front(_ u: Double, progress: Double) -> Double {
    let width = 0.8
    let position = -width + min(1, max(0, progress)) * (1 + 2 * width)
    return smoothstep((position - u) / width + 0.5)
  }

  private static func smoothstep(_ x: Double) -> Double {
    let t = min(1, max(0, x))
    return t * t * (3 - 2 * t)
  }
}
