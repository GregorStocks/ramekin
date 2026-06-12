import Foundation

/// Helpers for recipe export downloads: parsing the filename a server suggests
/// via Content-Disposition, picking sensible fallbacks, and writing the bytes
/// to a temp file so they can be handed to the iOS share sheet.
enum RecipeExportSupport {
    /// Parse the `filename=...` (or RFC 5987 `filename*=UTF-8''...`) attribute
    /// out of a Content-Disposition header value. Returns nil if the header is
    /// missing, malformed, or the filename can't be safely extracted.
    static func parseContentDispositionFilename(_ header: String?) -> String? {
        guard let header, !header.isEmpty else { return nil }

        if let star = matchAttribute(header, name: "filename*") {
            if let decoded = decodeRFC5987(star) {
                return sanitizeFilename(decoded)
            }
        }
        if let plain = matchAttribute(header, name: "filename") {
            return sanitizeFilename(plain)
        }
        return nil
    }

    /// Pick the filename to use for a downloaded export. Prefers what the
    /// server suggested via Content-Disposition; falls back to the provided
    /// default if the header is missing or malformed.
    static func suggestedFilename(from contentDisposition: String?, fallback: String) -> String {
        parseContentDispositionFilename(contentDisposition) ?? fallback
    }

    static func fallbackSingleRecipeFilename(now: Date = Date()) -> String {
        "recipe-\(timestamp(now)).paprikarecipe"
    }

    static func fallbackAllRecipesFilename(now: Date = Date()) -> String {
        "recipes-\(timestamp(now)).paprikarecipes"
    }

    /// Write `data` to a freshly-created path under `FileManager.default.temporaryDirectory`
    /// using `filename` as the visible name (so iOS share sheet shows it).
    static func writeToTempFile(
        data: Data,
        filename: String,
        fileManager: FileManager = .default
    ) throws -> URL {
        let safeName = sanitizeFilename(filename) ?? filename
        let directory = fileManager.temporaryDirectory.appendingPathComponent(
            "ramekin-exports/\(UUID().uuidString)",
            isDirectory: true
        )
        try fileManager.createDirectory(at: directory, withIntermediateDirectories: true)
        let fileURL = directory.appendingPathComponent(safeName)
        try data.write(to: fileURL, options: .atomic)
        return fileURL
    }

    // MARK: - Internal helpers

    private static func timestamp(_ date: Date) -> String {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = TimeZone.current
        formatter.dateFormat = "yyyyMMdd-HHmmss"
        return formatter.string(from: date)
    }

    private static func matchAttribute(_ header: String, name: String) -> String? {
        // Split on `;`, look for `name=value` where value may be quoted. Trim
        // whitespace, strip surrounding double-quotes, and return raw value.
        let parts = header.split(separator: ";").map { $0.trimmingCharacters(in: .whitespaces) }
        let prefix = name.lowercased() + "="
        for part in parts where part.lowercased().hasPrefix(prefix) {
            let value = String(part.dropFirst(prefix.count))
            if value.hasPrefix("\""), value.hasSuffix("\""), value.count >= 2 {
                return String(value.dropFirst().dropLast())
            }
            return value
        }
        return nil
    }

    private static func decodeRFC5987(_ value: String) -> String? {
        // Format: charset'language'percent-encoded-value
        let components = value.split(separator: "'", maxSplits: 2, omittingEmptySubsequences: false)
        guard components.count == 3 else { return nil }
        let charset = components[0].lowercased()
        guard charset == "utf-8" else { return nil }
        return components[2].removingPercentEncoding
    }

    private static func sanitizeFilename(_ name: String) -> String? {
        // Strip path components so a malicious server can't write outside the
        // temp directory we created for it. Drop control characters and any
        // path separators. Refuse an empty result.
        let lastComponent = (name as NSString).lastPathComponent
        let stripped = lastComponent.unicodeScalars.filter { scalar in
            scalar.value >= 0x20 && scalar != "/" && scalar != "\\" && scalar != ":"
        }
        let result = String(String.UnicodeScalarView(stripped))
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return result.isEmpty ? nil : result
    }
}
