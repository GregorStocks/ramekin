import Foundation
import os.log
import UniformTypeIdentifiers

private let sharedURLExtractorLogger = Logger(
    subsystem: "com.ramekin.app.share",
    category: "SharedURLExtractor"
)

protocol SharedURLItemProvider {
    var registeredTypeIdentifiers: [String] { get }

    func hasItemConformingToTypeIdentifier(_ typeIdentifier: String) -> Bool
    func loadItem(
        forTypeIdentifier typeIdentifier: String,
        completionHandler: @escaping @Sendable (NSSecureCoding?, Error?) -> Void
    )
}

extension NSItemProvider: SharedURLItemProvider {
    func loadItem(
        forTypeIdentifier typeIdentifier: String,
        completionHandler: @escaping @Sendable (NSSecureCoding?, Error?) -> Void
    ) {
        loadItem(forTypeIdentifier: typeIdentifier, options: nil) { item, error in
            completionHandler(item, error)
        }
    }
}

enum SharedURLExtractor {
    static func extractURL(from extensionItems: [NSExtensionItem]) async -> URL? {
        let providers = extensionItems.flatMap { item in
            item.attachments?.map { $0 as any SharedURLItemProvider } ?? []
        }
        return await extractURL(from: providers)
    }

    static func extractURL(from providers: [any SharedURLItemProvider]) async -> URL? {
        for provider in providers {
            sharedURLExtractorLogger.debug(
                "Provider registered types: \(provider.registeredTypeIdentifiers)"
            )

            if let url = await extractURL(from: provider) {
                return url
            }
        }

        sharedURLExtractorLogger.error("Could not extract URL from any provider")
        return nil
    }

    private static func extractURL(from provider: any SharedURLItemProvider) async -> URL? {
        if provider.hasItemConformingToTypeIdentifier(UTType.url.identifier) {
            sharedURLExtractorLogger.info("Provider has URL type, loading...")
            let (item, error) = await loadItem(
                from: provider,
                typeIdentifier: UTType.url.identifier
            )

            if let error {
                sharedURLExtractorLogger.error("Error loading URL: \(error.localizedDescription)")
            }
            if let url = item as? URL {
                sharedURLExtractorLogger.info(
                    "Successfully extracted URL: \(url.absoluteString)"
                )
                return url
            }
            if let url = item as? NSURL {
                sharedURLExtractorLogger.info(
                    "Successfully extracted URL: \(url.absoluteString ?? "")"
                )
                return url as URL
            }

            sharedURLExtractorLogger.warning("URL item was not a URL type")
        }

        if provider.hasItemConformingToTypeIdentifier(UTType.plainText.identifier) {
            sharedURLExtractorLogger.info("Provider has plainText type, loading...")
            let (item, error) = await loadItem(
                from: provider,
                typeIdentifier: UTType.plainText.identifier
            )

            if let error {
                sharedURLExtractorLogger.error("Error loading text: \(error.localizedDescription)")
            }
            if let text = item as? String,
               let url = URL(string: text.trimmingCharacters(in: .whitespacesAndNewlines)) {
                sharedURLExtractorLogger.info(
                    "Successfully extracted URL from text: \(url.absoluteString)"
                )
                return url
            }
            if let text = item as? NSString,
               let url = URL(
                string: text.trimmingCharacters(in: .whitespacesAndNewlines)
               ) {
                sharedURLExtractorLogger.info(
                    "Successfully extracted URL from text: \(url.absoluteString)"
                )
                return url
            }

            sharedURLExtractorLogger.warning("Text item was not a valid URL string")
        }

        return nil
    }

    private static func loadItem(
        from provider: any SharedURLItemProvider,
        typeIdentifier: String
    ) async -> (NSSecureCoding?, Error?) {
        await withCheckedContinuation { continuation in
            provider.loadItem(forTypeIdentifier: typeIdentifier) { item, error in
                continuation.resume(returning: (item, error))
            }
        }
    }
}
