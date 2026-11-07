import UIKit
import SwiftUI
import UniformTypeIdentifiers

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
            DebugLogger.shared.log("ERROR: No extension items found in context", source: "ShareExtension")
            completion(nil)
            return
        }
        DebugLogger.shared.log("Found \(extensionItems.count) extension items", source: "ShareExtension")
        Task {
            completion(await SharedPagePayloadExtractor.extractPayload(from: extensionItems))
        }
    }
}

/// SwiftUI View for the Share Extension. The state machine lives in
/// `ShareCaptureModel`; this view only renders its status.
struct ShareExtensionView: View {
    let payload: SharedPagePayload?

    @StateObject private var model: ShareCaptureModel

    init(payload: SharedPagePayload?, onComplete: @escaping () -> Void, onCancel: @escaping () -> Void) {
        self.payload = payload
        _model = StateObject(wrappedValue: ShareCaptureModel(
            payload: payload,
            onComplete: onComplete,
            onCancel: onCancel
        ))
    }

    var body: some View {
        NavigationStack {
            VStack(spacing: 24) {
                statusIcon
                    .scaledIconFont(size: 64)
                    .accessibilityHidden(true)
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
                switch model.status {
                case .success:
                    ToolbarItem(placement: .confirmationAction) {
                        Button("Done") { model.complete() }
                    }
                case .notLoggedIn:
                    ToolbarItem(placement: .confirmationAction) {
                        Button("OK") { model.cancel() }
                    }
                case .ready, .sending, .failed:
                    ToolbarItem(placement: .cancellationAction) {
                        Button("Cancel") { model.cancel() }
                    }
                }
            }
            .onAppear {
                model.start()
            }
        }
    }

    @ViewBuilder
    private var statusIcon: some View {
        switch model.status {
        case .ready, .sending:
            ProgressView()
                .scaleEffect(1.5)
        case .success:
            Image(systemName: "checkmark.circle.fill")
                .foregroundColor(.green)
        case .failed:
            Image(systemName: "xmark.circle.fill")
                .foregroundColor(.red)
        case .notLoggedIn:
            Image(systemName: "person.crop.circle.badge.exclamationmark.fill")
                .foregroundColor(.orange)
        }
    }

    @ViewBuilder
    private var statusText: some View {
        switch model.status {
        case .ready:
            Text("Preparing…")
                .font(.title2)
        case .sending:
            VStack(spacing: 8) {
                Text("Saving Recipe…")
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
        case .failed(let message):
            VStack(spacing: 8) {
                Text("Failed to Save")
                    .font(.title2)
                    .fontWeight(.bold)
                Text(message)
                    .font(.subheadline)
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
            }
        case .notLoggedIn:
            VStack(spacing: 8) {
                Text("Not Signed In")
                    .font(.title2)
                    .fontWeight(.bold)
                Text("Your session may have expired. Open the Ramekin app to sign in, then try again.")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
            }
        }
    }

    /// Finishing actions (Done, OK, Cancel) live in the navigation bar; the
    /// body only offers what the bar can't: retrying, or leaving a slow save.
    @ViewBuilder
    private var actionButton: some View {
        switch model.status {
        case .sending(slow: true):
            Button {
                model.complete()
            } label: {
                Text("Still working, tap to close")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .controlSize(.large)
        case .failed:
            Button {
                model.retry()
            } label: {
                Text("Try Again")
                    .fontWeight(.semibold)
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
        case .ready, .sending, .success, .notLoggedIn:
            EmptyView()
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
