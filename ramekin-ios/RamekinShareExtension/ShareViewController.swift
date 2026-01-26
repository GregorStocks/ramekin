import UIKit
import SwiftUI
import UniformTypeIdentifiers
import os.log

private let logger = Logger(subsystem: "com.ramekin.app.share", category: "ShareExtension")

/// Share Extension entry point
/// Handles receiving URLs from Safari and other apps
class ShareViewController: UIViewController {

    override init(nibName nibNameOrNil: String?, bundle nibBundleOrNil: Bundle?) {
        super.init(nibName: nibNameOrNil, bundle: nibBundleOrNil)
        DebugLogger.shared.log("ShareViewController init called")
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        DebugLogger.shared.log("ShareViewController init(coder:) called")
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        DebugLogger.shared.log("ShareViewController viewDidLoad called")
        logger.info("ShareViewController viewDidLoad called")

        // Extract the shared URL
        extractURL { [weak self] url in
            DispatchQueue.main.async {
                self?.presentShareView(with: url)
            }
        }
    }

    private func presentShareView(with url: URL?) {
        let shareView = ShareExtensionView(
            sharedURL: url,
            onComplete: { [weak self] in
                self?.extensionContext?.completeRequest(returningItems: nil)
            },
            onCancel: { [weak self] in
                self?.extensionContext?.cancelRequest(withError: NSError(
                    domain: "com.ramekin.share",
                    code: 0,
                    userInfo: [NSLocalizedDescriptionKey: "User cancelled"]
                ))
            }
        )

        let hostingController = UIHostingController(rootView: shareView)
        hostingController.view.backgroundColor = .systemBackground

        addChild(hostingController)
        view.addSubview(hostingController.view)

        hostingController.view.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            hostingController.view.topAnchor.constraint(equalTo: view.topAnchor),
            hostingController.view.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            hostingController.view.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            hostingController.view.trailingAnchor.constraint(equalTo: view.trailingAnchor)
        ])

        hostingController.didMove(toParent: self)
    }

    /// Extract URL from the share extension context
    private func extractURL(completion: @escaping (URL?) -> Void) {
        guard let extensionItems = extensionContext?.inputItems as? [NSExtensionItem] else {
            logger.error("No extension items found in context")
            completion(nil)
            return
        }

        logger.info("Found \(extensionItems.count) extension items")

        for item in extensionItems {
            guard let attachments = item.attachments else {
                logger.debug("Item has no attachments")
                continue
            }

            logger.info("Item has \(attachments.count) attachments")

            for provider in attachments {
                logger.debug("Provider registered types: \(provider.registeredTypeIdentifiers)")

                // Try to get URL directly
                if provider.hasItemConformingToTypeIdentifier(UTType.url.identifier) {
                    logger.info("Provider has URL type, loading...")
                    provider.loadItem(forTypeIdentifier: UTType.url.identifier) { item, error in
                        if let error = error {
                            logger.error("Error loading URL: \(error.localizedDescription)")
                        }
                        if let url = item as? URL {
                            logger.info("Successfully extracted URL: \(url.absoluteString)")
                            completion(url)
                            return
                        }
                        logger.warning("URL item was not a URL type")
                    }
                    return
                }

                // Try to get plain text (might be a URL string)
                if provider.hasItemConformingToTypeIdentifier(UTType.plainText.identifier) {
                    logger.info("Provider has plainText type, loading...")
                    provider.loadItem(forTypeIdentifier: UTType.plainText.identifier) { item, error in
                        if let error = error {
                            logger.error("Error loading text: \(error.localizedDescription)")
                        }
                        if let text = item as? String, let url = URL(string: text) {
                            logger.info("Successfully extracted URL from text: \(url.absoluteString)")
                            completion(url)
                            return
                        }
                        logger.warning("Text item was not a valid URL string")
                    }
                    return
                }
            }
        }

        logger.error("Could not extract URL from any provider")
        completion(nil)
    }
}

/// SwiftUI View for the Share Extension
struct ShareExtensionView: View {
    let sharedURL: URL?
    let onComplete: () -> Void
    let onCancel: () -> Void

    @State private var status: ShareStatus = .ready
    @State private var errorMessage: String?

    enum ShareStatus {
        case ready
        case sending
        case success
        case error
        case notLoggedIn
    }

    var body: some View {
        NavigationStack {
            VStack(spacing: 24) {
                statusIcon
                    .font(.system(size: 64))
                    .padding(.top, 32)

                statusText

                if let url = sharedURL {
                    Text(url.absoluteString)
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .lineLimit(2)
                        .multilineTextAlignment(.center)
                        .padding(.horizontal)
                }

                Spacer()

                actionButton
                    .padding(.horizontal)
                    .padding(.bottom, 32)
            }
            .navigationTitle("Save to Ramekin")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") {
                        onCancel()
                    }
                }
            }
            .onAppear {
                checkLoginAndSend()
            }
        }
    }

    @ViewBuilder
    private var statusIcon: some View {
        switch status {
        case .ready, .sending:
            ProgressView()
                .scaleEffect(1.5)
        case .success:
            Image(systemName: "checkmark.circle.fill")
                .foregroundColor(.green)
        case .error:
            Image(systemName: "xmark.circle.fill")
                .foregroundColor(.red)
        case .notLoggedIn:
            Image(systemName: "person.crop.circle.badge.exclamationmark.fill")
                .foregroundColor(.orange)
        }
    }

    @ViewBuilder
    private var statusText: some View {
        switch status {
        case .ready:
            Text("Preparing...")
                .font(.title2)
        case .sending:
            VStack(spacing: 8) {
                Text("Saving Recipe...")
                    .font(.title2)
                Text("The recipe will be processed in the background")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
            }
        case .success:
            VStack(spacing: 8) {
                Text("Saved!")
                    .font(.title2)
                    .fontWeight(.bold)
                Text("The recipe is being imported")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
            }
        case .error:
            VStack(spacing: 8) {
                Text("Failed to Save")
                    .font(.title2)
                    .fontWeight(.bold)
                if let error = errorMessage {
                    Text(error)
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                        .multilineTextAlignment(.center)
                }
            }
        case .notLoggedIn:
            VStack(spacing: 8) {
                Text("Not Signed In")
                    .font(.title2)
                    .fontWeight(.bold)
                Text("Open the Ramekin app to sign in first")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
            }
        }
    }

    @ViewBuilder
    private var actionButton: some View {
        switch status {
        case .ready, .sending:
            EmptyView()
        case .success:
            Button {
                onComplete()
            } label: {
                Text("Done")
                    .fontWeight(.semibold)
                    .frame(maxWidth: .infinity)
                    .padding()
                    .background(Color.green)
                    .foregroundColor(.white)
                    .cornerRadius(12)
            }
        case .error:
            VStack(spacing: 12) {
                Button {
                    checkLoginAndSend()
                } label: {
                    Text("Try Again")
                        .fontWeight(.semibold)
                        .frame(maxWidth: .infinity)
                        .padding()
                        .background(Color.orange)
                        .foregroundColor(.white)
                        .cornerRadius(12)
                }

                Button {
                    onCancel()
                } label: {
                    Text("Cancel")
                        .foregroundColor(.secondary)
                }
            }
        case .notLoggedIn:
            Button {
                onCancel()
            } label: {
                Text("OK")
                    .fontWeight(.semibold)
                    .frame(maxWidth: .infinity)
                    .padding()
                    .background(Color.orange)
                    .foregroundColor(.white)
                    .cornerRadius(12)
            }
        }
    }

    private func checkLoginAndSend() {
        DebugLogger.shared.log("checkLoginAndSend called")
        DebugLogger.shared.log("isLoggedIn: \(RamekinAPI.shared.isLoggedIn)")
        DebugLogger.shared.log("serverURL: \(RamekinAPI.shared.serverURL ?? "nil")")
        DebugLogger.shared.log("authToken present: \(RamekinAPI.shared.authToken != nil)")
        logger.info("checkLoginAndSend called, isLoggedIn: \(RamekinAPI.shared.isLoggedIn)")

        // Check if logged in
        guard RamekinAPI.shared.isLoggedIn else {
            DebugLogger.shared.log("ERROR: User not logged in")
            logger.warning("User not logged in")
            status = .notLoggedIn
            return
        }

        // Check if we have a URL
        guard let url = sharedURL else {
            DebugLogger.shared.log("ERROR: No URL provided to share")
            logger.error("No URL provided to share")
            status = .error
            errorMessage = "No URL to save"
            return
        }

        DebugLogger.shared.log("URL to share: \(url.absoluteString)")
        // Send the URL
        sendURL(url)
    }

    private func sendURL(_ url: URL) {
        DebugLogger.shared.log("sendURL called with: \(url.absoluteString)")
        logger.info("Sending URL to API: \(url.absoluteString)")
        status = .sending

        Task {
            do {
                DebugLogger.shared.log("Calling RamekinAPI.shared.scrapeURL...")
                _ = try await RamekinAPI.shared.scrapeURL(url.absoluteString)
                DebugLogger.shared.log("API call completed successfully")
                logger.info("API call succeeded")

                await MainActor.run {
                    status = .success
                    DebugLogger.shared.log("Status set to success, will dismiss in 1.5s")

                    // Auto-dismiss after a short delay
                    DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
                        DebugLogger.shared.log("Calling onComplete()")
                        onComplete()
                    }
                }
            } catch {
                DebugLogger.shared.log("API call FAILED: \(error)")
                DebugLogger.shared.log("Error localized: \(error.localizedDescription)")
                logger.error("API call failed: \(error.localizedDescription)")
                await MainActor.run {
                    status = .error
                    errorMessage = error.localizedDescription
                }
            }
        }
    }
}

#Preview {
    ShareExtensionView(
        sharedURL: URL(string: "https://example.com/recipe"),
        onComplete: {},
        onCancel: {}
    )
}
