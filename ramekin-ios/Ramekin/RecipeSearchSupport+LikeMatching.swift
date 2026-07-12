import Foundation

/// The mirror of PostgreSQL `ILIKE` over contract-normalized codepoints.
/// Plain substring containment is not enough: the server escapes LIKE
/// metacharacters and *then* unaccents the pattern, and a few unaccent
/// replacements emit `%`, `_`, or `\`, so those act as wildcards or escapes
/// in the final pattern (pinned by the fullwidth-percent shared vector case).
extension RecipeSearchSupport {
    private static let percent: UInt32 = 0x25
    private static let underscore: UInt32 = 0x5F
    private static let backslash: UInt32 = 0x5C

    /// One bare-text token's LIKE pattern, built exactly like the server:
    /// escape `\`, `%`, `_`, then normalize (which may introduce new,
    /// unescaped metacharacters).
    static func membershipPattern(_ token: String) -> [UInt32] {
        let escaped = token
            .replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "%", with: "\\%")
            .replacingOccurrences(of: "_", with: "\\_")
        return normalizedScalars(escaped)
    }

    /// `haystack ILIKE '%' || pattern || '%'` over already-normalized
    /// codepoints: `%` matches any run, `_` exactly one codepoint, `\` makes
    /// the next codepoint literal.
    static func likeContains(haystack: [UInt32], pattern: [UInt32]) -> Bool {
        var fullPattern: [UInt32] = [percent]
        fullPattern.append(contentsOf: pattern)
        fullPattern.append(percent)
        return likeMatches(haystack: haystack, pattern: fullPattern)
    }

    private static func likeMatches(haystack: [UInt32], pattern: [UInt32]) -> Bool {
        var textIndex = 0
        var patternIndex = 0
        var starPattern = -1
        var starText = -1

        while textIndex < haystack.count {
            if patternIndex < pattern.count, pattern[patternIndex] == percent {
                starPattern = patternIndex
                starText = textIndex
                patternIndex += 1
            } else if patternIndex < pattern.count, pattern[patternIndex] == underscore {
                patternIndex += 1
                textIndex += 1
            } else if patternIndex < pattern.count,
                      literal(in: pattern, at: patternIndex).scalar == haystack[textIndex] {
                patternIndex += literal(in: pattern, at: patternIndex).width
                textIndex += 1
            } else if starPattern >= 0 {
                starText += 1
                textIndex = starText
                patternIndex = starPattern + 1
            } else {
                return false
            }
        }

        while patternIndex < pattern.count, pattern[patternIndex] == percent {
            patternIndex += 1
        }
        return patternIndex == pattern.count
    }

    private static func literal(in pattern: [UInt32], at index: Int) -> (scalar: UInt32, width: Int) {
        if pattern[index] == backslash, index + 1 < pattern.count {
            return (pattern[index + 1], 2)
        }
        return (pattern[index], 1)
    }
}
