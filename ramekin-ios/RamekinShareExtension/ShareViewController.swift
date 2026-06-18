import UIKit
import SwiftUI
import UniformTypeIdentifiers
import os.log

private let logger = Logger(subsystem: "com.ramekin.app.share", category: "ShareExtension")

/// Share Extension entry point.
///
/// We only run from Safari via a JS preprocessing file (see
/// `SharePreprocessor.js` and Info.plist). The preprocessor delivers a
/// property-list payload containing `{html, url, title}`; we POST that to
/// `/api/scrape/capture`, matching the web bookmarklet.
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

        extractPayload { [weak self] payload in
            DispatchQueue.main.async {
                self?.presentShareView(with: payload)
            }
        }
    }

    private func presentShareView(with payload: SharedPagePayload?) {
        let shareView = ShareExtensionView(
            payload: payload,
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

    private func extractPayload(completion: @escaping (SharedPagePayload?) -> Void) {
        guard let extensionItems = extensionContext?.inputItems as? [NSExtensionItem] else {
            logger.error("No extension items found in context")
            completion(nil)
            return
        }
        logger.info("Found \(extensionItems.count) extension items")
        Task {
            completion(await SharedPagePayloadExtractor.extractPayload(from: extensionItems))
        }
    }
}

/// SwiftUI View for the Share Extension
struct ShareExtensionView: View {
    let payload: SharedPagePayload?
    let onComplete: () -> Void
    let onCancel: () -> Void

    @State private var status: ShareStatus = .ready
    @State private var errorMessage: String?
    @State private var showSlowAffordance = false

    enum ShareStatus {
        case ready
        case sending
        case success
        case error
        case notLoggedIn
    }

    /// Delay after which, if we're still in `.sending`, we surface a
    /// "Still working, tap to close" affordance so the user can dismiss
    /// instead of waiting for iOS to terminate the extension.
    static let slowAffordanceDelay: TimeInterval = 10

    var body: some View {
        NavigationStack {
            VStack(spacing: 24) {
                statusIcon
                    .font(.system(size: 64))
                    .padding(.top, 32)

                statusText

                if let payload {
                    Text(payload.url.absoluteString)
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
        case .ready:
            EmptyView()
        case .sending:
            if showSlowAffordance {
                Button {
                    onComplete()
                } label: {
                    Text("Still working, tap to close")
                        .fontWeight(.semibold)
                        .frame(maxWidth: .infinity)
                        .padding()
                        .background(Color.secondary.opacity(0.2))
                        .foregroundColor(.primary)
                        .cornerRadius(12)
                }
            } else {
                EmptyView()
            }
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

        guard RamekinAPI.shared.isLoggedIn else {
            DebugLogger.shared.log("ERROR: User not logged in")
            logger.warning("User not logged in")
            status = .notLoggedIn
            return
        }

        guard let payload else {
            DebugLogger.shared.log("ERROR: No payload from Safari preprocessing")
            logger.error("No payload from Safari preprocessing")
            status = .error
            errorMessage = "Could not read page content. Open the page in Safari and share from there."
            return
        }

        DebugLogger.shared.log("Payload URL: \(payload.url.absoluteString)")
        sendCapture(payload)
    }

    private func sendCapture(_ payload: SharedPagePayload) {
        DebugLogger.shared.log("sendCapture for: \(payload.url.absoluteString)")
        logger.info("Sending capture to API: \(payload.url.absoluteString)")
        status = .sending
        showSlowAffordance = false

        Task {
            try? await Task.sleep(nanoseconds: UInt64(Self.slowAffordanceDelay * 1_000_000_000))
            await MainActor.run {
                if status == .sending {
                    DebugLogger.shared.log(
                        "Still sending after \(Int(Self.slowAffordanceDelay))s - showing slow affordance",
                        source: "ShareExtension"
                    )
                    showSlowAffordance = true
                }
            }
        }

        Task {
            do {
                _ = try await DebugLogger.shared.timed("captureHTML", source: "ShareExtension") {
                    try await RamekinAPI.shared.captureHTML(
                        html: payload.html,
                        sourceURL: payload.url.absoluteString
                    )
                }
                logger.info("API call succeeded")

                await MainActor.run {
                    status = .success
                    DebugLogger.shared.log("Status set to success, will dismiss in 1.5s", source: "ShareExtension")
                    DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
                        DebugLogger.shared.log("Calling onComplete()", source: "ShareExtension")
                        onComplete()
                    }
                }
            } catch {
                DebugLogger.shared.log("API call FAILED: \(error)", source: "ShareExtension")
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
        payload: SharedPagePayload(
            html: "<html><body>preview</body></html>",
            url: URL(string: "https://example.com/recipe")!,
            title: "Example"
        ),
        onComplete: {},
        onCancel: {}
    )
}
