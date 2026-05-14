import SwiftUI

private let defaultServerURL = "https://ramekin.app"

struct LoginView: View {
    @EnvironmentObject var appState: AppState

    @State private var serverURL: String = defaultServerURL
    @State private var username: String = "t"
    @State private var password: String = "t"
    @State private var accessClientId: String = ""
    @State private var accessClientSecret: String = ""

    @State private var isLoading: Bool = false
    @State private var errorMessage: String?

    @FocusState private var focusedField: Field?

    enum Field {
        case serverURL, username, password, accessClientId, accessClientSecret
    }

    var body: some View {
        Form {
            Section {
                VStack(spacing: 16) {
                    Image(systemName: "fork.knife.circle.fill")
                        .font(.system(size: 64))
                        .foregroundColor(.orange)

                    Text("Ramekin")
                        .font(.largeTitle)
                        .fontWeight(.bold)

                    Text("Sign in to save recipes from Safari")
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                        .multilineTextAlignment(.center)
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 24)
                .listRowBackground(Color.clear)
            }

            Section("Server") {
                TextField(defaultServerURL, text: $serverURL)
                    .textContentType(.URL)
                    .keyboardType(.URL)
                    .autocapitalization(.none)
                    .autocorrectionDisabled()
                    .focused($focusedField, equals: .serverURL)
                    .submitLabel(.next)
                    .onSubmit { focusedField = .username }
            }

            Section("Credentials") {
                TextField("Username", text: $username)
                    .textContentType(.username)
                    .autocapitalization(.none)
                    .autocorrectionDisabled()
                    .focused($focusedField, equals: .username)
                    .submitLabel(.next)
                    .onSubmit { focusedField = .password }

                SecureField("Password", text: $password)
                    .textContentType(.password)
                    .focused($focusedField, equals: .password)
                    .submitLabel(.next)
                    .onSubmit { focusedField = .accessClientId }
            }

            Section("Cloudflare Access (optional)") {
                TextField("Client ID", text: $accessClientId)
                    .autocapitalization(.none)
                    .autocorrectionDisabled()
                    .focused($focusedField, equals: .accessClientId)
                    .submitLabel(.next)
                    .onSubmit { focusedField = .accessClientSecret }

                SecureField("Client Secret", text: $accessClientSecret)
                    .focused($focusedField, equals: .accessClientSecret)
                    .submitLabel(.go)
                    .onSubmit { login() }
            }

            Section {
                Button(action: login) {
                    HStack {
                        Spacer()
                        if isLoading {
                            ProgressView()
                                .progressViewStyle(CircularProgressViewStyle(tint: .white))
                        } else {
                            Text("Sign In")
                                .fontWeight(.semibold)
                        }
                        Spacer()
                    }
                }
                .disabled(isLoading || serverURL.isEmpty || username.isEmpty || password.isEmpty)
                .listRowBackground(
                    (isLoading || serverURL.isEmpty || username.isEmpty || password.isEmpty)
                        ? Color.gray
                        : Color.orange
                )
                .foregroundColor(.white)
            }

            if let error = errorMessage {
                Section {
                    HStack {
                        Image(systemName: "exclamationmark.triangle.fill")
                            .foregroundColor(.red)
                        Text(error)
                            .foregroundColor(.red)
                            .accessibilityIdentifier("login-error-message")
                    }
                }
            }
        }
        .navigationTitle("Sign In")
        .navigationBarTitleDisplayMode(.inline)
        .onAppear {
            // Pre-fill saved credentials if available
            if let savedURL = KeychainHelper.shared.getServerURL() {
                serverURL = savedURL
            }
            if let savedId = KeychainHelper.shared.getAccessClientId() {
                accessClientId = savedId
            }
            if let savedSecret = KeychainHelper.shared.getAccessClientSecret() {
                accessClientSecret = savedSecret
            }
        }
    }

    private func login() {
        guard !isLoading else { return }

        isLoading = true
        errorMessage = nil

        Task {
            do {
                _ = try await RamekinAPI.shared.login(
                    serverURL: serverURL,
                    username: username,
                    password: password,
                    accessClientId: accessClientId.isEmpty ? nil : accessClientId,
                    accessClientSecret: accessClientSecret.isEmpty ? nil : accessClientSecret
                )

                await MainActor.run {
                    isLoading = false
                    appState.refreshState()
                }
            } catch {
                await MainActor.run {
                    isLoading = false
                    errorMessage = error.localizedDescription
                }
            }
        }
    }
}

#Preview {
    NavigationStack {
        LoginView()
    }
    .environmentObject(AppState())
}
