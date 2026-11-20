import CoreGraphics
import Foundation

/// Reads the SVG path data the Anarlog wordmark uses (`M L H V C Z`, absolute or relative) into a
/// CGPath in the SVG's own coordinates, y pointing down.
public enum SVGPath {
  public enum ParseError: Error, Equatable {
    case unsupportedCommand(Character)
    case missingNumber
  }

  public static func parse(_ data: String) throws -> CGPath {
    var scanner = Tokens(data)
    let path = CGMutablePath()
    var current = CGPoint.zero
    var start = CGPoint.zero
    var command: Character?

    while let next = scanner.nextCommandOrNil() {
      if let letter = next { command = letter }
      guard let op = command else { throw ParseError.missingNumber }
      let relative = op.isLowercase
      func point() throws -> CGPoint {
        let x = try scanner.number()
        let y = try scanner.number()
        return relative ? CGPoint(x: current.x + x, y: current.y + y) : CGPoint(x: x, y: y)
      }
      switch op.uppercased().first! {
      case "M":
        current = try point()
        start = current
        path.move(to: current)
        // Pairs after a move are lines.
        command = relative ? "l" : "L"
      case "L":
        current = try point()
        path.addLine(to: current)
      case "H":
        let x = try scanner.number()
        current.x = relative ? current.x + x : x
        path.addLine(to: current)
      case "V":
        let y = try scanner.number()
        current.y = relative ? current.y + y : y
        path.addLine(to: current)
      case "C":
        let c1 = try point()
        let c2 = try point()
        let end = try point()
        path.addCurve(to: end, control1: c1, control2: c2)
        current = end
      case "Z":
        path.closeSubpath()
        current = start
        command = nil
      default:
        throw ParseError.unsupportedCommand(op)
      }
    }
    return path
  }

  private struct Tokens {
    private let chars: [Character]
    private var index = 0

    init(_ data: String) { chars = Array(data) }

    private mutating func skipSeparators() {
      while index < chars.count,
        chars[index] == " " || chars[index] == "," || chars[index].isNewline
          || chars[index] == "\t"
      { index += 1 }
    }

    /// `.some(letter)` at a command, `.some(nil)` at a number that repeats the last command,
    /// `nil` at the end.
    mutating func nextCommandOrNil() -> Character?? {
      skipSeparators()
      guard index < chars.count else { return nil }
      let c = chars[index]
      if c.isLetter {
        index += 1
        return .some(c)
      }
      return .some(nil)
    }

    mutating func number() throws -> CGFloat {
      skipSeparators()
      let begin = index
      if index < chars.count, chars[index] == "-" || chars[index] == "+" { index += 1 }
      var seenDot = false
      while index < chars.count {
        let c = chars[index]
        if c.isNumber {
          index += 1
        } else if c == ".", !seenDot {
          seenDot = true
          index += 1
        } else {
          break
        }
      }
      guard index > begin, let value = Double(String(chars[begin..<index])) else {
        throw ParseError.missingNumber
      }
      return CGFloat(value)
    }
  }
}
