import SwiftUI

/// Bridges the engine's `highlight` markers to a styled `AttributedString`.
///
/// `SearchModel` asks the engine to wrap each matching region in `open`/`close`
/// sentinels; the view turns that marked text into an `AttributedString` whose
/// matched spans stand out in the result list.
enum Highlight {
    /// Sentinel markers wrapped around matched regions. C0 control characters,
    /// so they never collide with real (normalized) content.
    static let open = "\u{2}" // STX
    static let close = "\u{3}" // ETX

    /// Parses text wrapped with `open`/`close` markers into an `AttributedString`,
    /// emphasizing the matched spans.
    static func attributed(_ marked: String) -> AttributedString {
        let openChar = Character(open)
        let closeChar = Character(close)

        var result = AttributedString()
        var span = AttributedString()
        var inMatch = false

        func flush() {
            guard !span.characters.isEmpty else { return }
            if inMatch {
                span.backgroundColor = .yellow.opacity(0.5)
                span.foregroundColor = .primary
                span.inlinePresentationIntent = .stronglyEmphasized
            }
            result.append(span)
            span = AttributedString()
        }

        for ch in marked {
            switch ch {
            case openChar:
                flush()
                inMatch = true
            case closeChar:
                flush()
                inMatch = false
            default:
                span.append(AttributedString(String(ch)))
            }
        }
        flush()
        return result
    }
}
