import Foundation

/// The versioned search normalization contract shared with the server
/// (shared-test-vectors/search-normalization.json): a complete
/// per-codepoint unaccent mapping and a per-codepoint lowercase mapping,
/// which the server's search and tag matching apply. Local search matches server result membership
/// only while both sides normalize with the same contract version, so recipe
/// sync fails when the versions differ (see RecipeSyncSweep).
enum SearchNormalizationSupport {
    static var contractVersion: Int { contract.version }

    /// Mirror of the server's `normalize_for_search` — the exact
    /// normalization server bare-text matching applies to both the recipe
    /// fields and the query, and the one the relevance scorer uses.
    /// Applied per Unicode scalar; unaccent replacements may be empty or
    /// multi-character. The lowercase mapping has no context rules (no
    /// final-sigma handling), so Swift's `lowercased()` must not be used.
    static func normalizeForSearch(_ text: String) -> String {
        var out = String.UnicodeScalarView()
        for scalar in text.unicodeScalars {
            if let replacement = contract.unaccent[scalar.value] {
                for replacementScalar in replacement.unicodeScalars {
                    out.append(lowered(replacementScalar))
                }
            } else {
                out.append(lowered(scalar))
            }
        }
        return String(out)
    }

    /// Mirror of the server's tag-name folding (`ramekin_core::search::
    /// fold_case`): the contract's per-codepoint lowercase mapping only. Tag
    /// comparisons are case-insensitive but accent-sensitive, so they must
    /// not pass through the unaccent mapping.
    static func citextFold(_ text: String) -> String {
        String(String.UnicodeScalarView(text.unicodeScalars.map(lowered)))
    }

    private static func lowered(_ scalar: Unicode.Scalar) -> Unicode.Scalar {
        contract.lower[scalar.value] ?? scalar
    }

    private struct Contract {
        let version: Int
        let unaccent: [UInt32: String]
        let lower: [UInt32: Unicode.Scalar]
    }

    private struct ContractFile: Decodable {
        let version: Int
        let unaccent: [String: String]
        let lower: [String: String]
    }

    private final class BundleToken {}

    private static let contract: Contract = {
        guard let url = Bundle(for: BundleToken.self).url(
            forResource: "search-normalization",
            withExtension: "json"
        ) else {
            fatalError("search-normalization.json is missing from the app bundle")
        }
        let file: ContractFile
        do {
            file = try JSONDecoder().decode(ContractFile.self, from: Data(contentsOf: url))
        } catch {
            fatalError("search-normalization.json is invalid: \(error)")
        }
        var unaccent: [UInt32: String] = [:]
        for (key, replacement) in file.unaccent {
            unaccent[codepoint(key)] = replacement
        }
        var lower: [UInt32: Unicode.Scalar] = [:]
        for (key, replacement) in file.lower {
            let scalars = Array(replacement.unicodeScalars)
            guard scalars.count == 1 else {
                fatalError("lower mapping for \(key) is not a single scalar")
            }
            lower[codepoint(key)] = scalars[0]
        }
        return Contract(version: file.version, unaccent: unaccent, lower: lower)
    }()

    private static func codepoint(_ key: String) -> UInt32 {
        guard let value = UInt32(key, radix: 16), Unicode.Scalar(value) != nil else {
            fatalError("invalid codepoint key in normalization contract: \(key)")
        }
        return value
    }
}
