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

    /// `normalizeForSearch` as UTF-8 bytes, for the search corpus: substring
    /// containment over the UTF-8 of two normalized strings is the same
    /// relation as over their Unicode scalars. Works on bytes because the
    /// whole cookbook passes through here whenever the corpus is rebuilt,
    /// and a per-scalar `String` round trip made that take seconds.
    static func normalizedUTF8(_ text: String) -> [UInt8] {
        var text = text
        text.makeContiguousUTF8()
        let count = text.utf8.count
        let scratch = UnsafeMutablePointer<UInt8>.allocate(capacity: count * contract.maxUTF8Expansion + 1)
        defer { scratch.deallocate() }
        var written = 0
        text.withCString { cString in
            let source = UnsafeRawPointer(cString).assumingMemoryBound(to: UInt8.self)
            var index = 0
            while index < count {
                // Most text is bytes normalization leaves alone; libc finds
                // the end of each such run far faster than a Swift loop.
                let run = min(strcspn(cString + index, contract.changedBytes), count - index)
                if run > 0 {
                    memcpy(scratch + written, source + index, run)
                    written += run
                    index += run
                    continue
                }
                let byte = source[index]
                if byte < 0x80 {
                    scratch[written] = contract.asciiTable[Int(byte)]
                    written += 1
                    index += 1
                    continue
                }
                // Swift strings are valid UTF-8, so the lead byte gives the
                // sequence length.
                let length = min(byte >= 0xF0 ? 4 : (byte >= 0xE0 ? 3 : 2), count - index)
                var value = UInt32(byte) & (0xFF >> (length + 1))
                for offset in 1..<length {
                    value = (value << 6) | UInt32(source[index + offset] & 0x3F)
                }
                if let replacement = contract.replacementUTF8[value] {
                    for replacementByte in replacement {
                        scratch[written] = replacementByte
                        written += 1
                    }
                } else {
                    memcpy(scratch + written, source + index, length)
                    written += length
                }
                index += length
            }
        }
        return Array(UnsafeBufferPointer(start: scratch, count: written))
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
        /// The normalized UTF-8 of every scalar normalization changes.
        let replacementUTF8: [UInt32: [UInt8]]
        /// Normalized byte for each ASCII byte.
        let asciiTable: [UInt8]
        /// Upper bound on normalized UTF-8 bytes per input UTF-8 byte.
        let maxUTF8Expansion: Int
        /// Every byte that can start something normalization changes, as a C
        /// string: the ASCII bytes `asciiTable` remaps and all non-ASCII
        /// bytes. (NUL can't be listed; a run simply ends at one.)
        let changedBytes: [CChar]
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
        return makeContract(version: file.version, unaccent: unaccent, lower: lower)
    }()

    /// Derives the byte-level tables `normalizedUTF8` runs on from the
    /// per-scalar mappings.
    private static func makeContract(
        version: Int,
        unaccent: [UInt32: String],
        lower: [UInt32: Unicode.Scalar]
    ) -> Contract {
        func normalized(_ scalar: Unicode.Scalar) -> [UInt8] {
            let replaced = unaccent[scalar.value].map { Array($0.unicodeScalars) } ?? [scalar]
            return Array(String(String.UnicodeScalarView(replaced.map { lower[$0.value] ?? $0 })).utf8)
        }
        var replacementUTF8: [UInt32: [UInt8]] = [:]
        var maxUTF8Expansion = 1
        for key in Set(unaccent.keys).union(lower.keys) where key >= 0x80 {
            guard let scalar = Unicode.Scalar(key) else { continue }
            let replacement = normalized(scalar)
            replacementUTF8[key] = replacement
            let sourceLength = String(scalar).utf8.count
            maxUTF8Expansion = max(maxUTF8Expansion, (replacement.count + sourceLength - 1) / sourceLength)
        }
        let asciiTable = (UInt8(0)..<0x80).map { byte -> UInt8 in
            let replacement = normalized(Unicode.Scalar(byte))
            guard replacement.count == 1 else {
                fatalError("normalization contract maps ASCII \(byte) to \(replacement.count) bytes")
            }
            return replacement[0]
        }
        return Contract(
            version: version,
            unaccent: unaccent,
            lower: lower,
            replacementUTF8: replacementUTF8,
            asciiTable: asciiTable,
            maxUTF8Expansion: maxUTF8Expansion,
            changedBytes: (UInt8(1)...0xFF)
                .filter { $0 >= 0x80 || asciiTable[Int($0)] != $0 }
                .map { CChar(bitPattern: $0) } + [0]
        )
    }

    private static func codepoint(_ key: String) -> UInt32 {
        guard let value = UInt32(key, radix: 16), Unicode.Scalar(value) != nil else {
            fatalError("invalid codepoint key in normalization contract: \(key)")
        }
        return value
    }
}
