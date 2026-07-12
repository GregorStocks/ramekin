import SwiftUI
import UIKit

private extension Notification.Name {
    static let universalLinkReceived = Notification.Name("RamekinUniversalLinkReceived")
}

private final class UniversalLinkStore {
    static let shared = UniversalLinkStore()

    private var pendingURL: URL?

    private init() {}

    func submit(_ url: URL) {
        pendingURL = url
        NotificationCenter.default.post(name: .universalLinkReceived, object: url)
    }

    func consumePendingURL() -> URL? {
        defer { pendingURL = nil }
        return pendingURL
    }
}

final class AppDelegate: NSObject, UIApplicationDelegate {
    func application(
        _ application: UIApplication,
        continue userActivity: NSUserActivity,
        restorationHandler: @escaping ([UIUserActivityRestoring]?) -> Void
    ) -> Bool {
        guard userActivity.activityType == NSUserActivityTypeBrowsingWeb,
              let url = userActivity.webpageURL else {
            return false
        }
        UniversalLinkStore.shared.submit(url)
        return true
    }
}

@main
struct RamekinApp: App {
    @UIApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @StateObject private var appState = AppState()

    init() {
        // Keychain state survives reinstalls on the simulator, so UI tests
        // pass this argument to guarantee they start from the login screen.
        if ProcessInfo.processInfo.arguments.contains("--uitest-reset-auth") {
            RamekinAPI.shared.logout()
        }
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(appState)
        }
    }
}

/// Global app state shared across views
@MainActor
class AppState: ObservableObject {
    @Published var isLoggedIn: Bool = false
    @Published var username: String = ""
    @Published var serverURL: String = ""
    @Published var pendingRecipeId: UUID?
    private var lastHandledUniversalLink: URL?
    private var lastHandledUniversalLinkAt = Date.distantPast
    private var universalLinkObserver: NSObjectProtocol?
    private var authExpiredObserver: NSObjectProtocol?

    init() {
        refreshState()
        universalLinkObserver = NotificationCenter.default.addObserver(
            forName: .universalLinkReceived,
            object: nil,
            queue: .main
        ) { [weak self] notification in
            guard let url = notification.object as? URL else { return }
            Task { @MainActor [weak self] in
                self?.handleUniversalLink(url)
            }
        }
        authExpiredObserver = NotificationCenter.default.addObserver(
            forName: .ramekinAuthExpired,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor [weak self] in
                self?.handleAuthExpired()
            }
        }
        if let url = UniversalLinkStore.shared.consumePendingURL() {
            handleUniversalLink(url)
        }
    }

    deinit {
        if let universalLinkObserver {
            NotificationCenter.default.removeObserver(universalLinkObserver)
        }
        if let authExpiredObserver {
            NotificationCenter.default.removeObserver(authExpiredObserver)
        }
    }

    private func handleAuthExpired() {
        // Only act when we still think we're logged in — repeated 401s in a
        // single burst shouldn't each schedule a redundant logout.
        guard isLoggedIn else { return }
        logout()
    }

    func refreshState() {
        isLoggedIn = RamekinAPI.shared.isLoggedIn
        username = KeychainHelper.shared.getUsername() ?? ""
        serverURL = KeychainHelper.shared.getServerURL() ?? ""
        let accountKey = AccountScope.currentAccountKey()
        TagFilterCache.migrateLegacyState(activeAccountKey: accountKey)
        ShoppingListStore.shared.setActiveAccountKey(accountKey)
    }

    func logout() {
        RamekinAPI.shared.logout()
        refreshState()
    }

    func handleUniversalLink(_ url: URL) {
        guard let components = URLComponents(url: url, resolvingAgainstBaseURL: false) else {
            return
        }
        let parts = components.path.split(separator: "/", omittingEmptySubsequences: true)
        guard parts.count == 2, parts[0] == "recipes", let id = UUID(uuidString: String(parts[1])) else {
            return
        }
        let now = Date()
        if lastHandledUniversalLink == url, now.timeIntervalSince(lastHandledUniversalLinkAt) < 1 {
            return
        }
        lastHandledUniversalLink = url
        lastHandledUniversalLinkAt = now
        pendingRecipeId = id
    }
}

enum Tab: Hashable {
    case recipes
    case mealPlan
    case shopping
}

struct ContentView: View {
    @EnvironmentObject var appState: AppState
    @State private var selectedTab: Tab = .recipes

    var body: some View {
        Group {
            if appState.isLoggedIn {
                TabView(selection: $selectedTab) {
                    RecipesTab()
                        .tabItem {
                            Label("Recipes", systemImage: "book")
                        }
                        .tag(Tab.recipes)

                    MealPlanView()
                        .tabItem {
                            Label("Meal Plan", systemImage: "calendar")
                        }
                        .tag(Tab.mealPlan)

                    ShoppingListView()
                        .tabItem {
                            Label("Shopping", systemImage: "cart")
                        }
                        .tag(Tab.shopping)
                }
                .onChange(of: appState.pendingRecipeId) { id in
                    if id != nil { selectedTab = .recipes }
                }
            } else {
                LoginView()
            }
        }
        .onContinueUserActivity(NSUserActivityTypeBrowsingWeb) { activity in
            guard let url = activity.webpageURL else { return }
            appState.handleUniversalLink(url)
        }
    }
}

struct RecipesTab: View {
    @EnvironmentObject var appState: AppState
    @State private var path = NavigationPath()

    var body: some View {
        NavigationStack(path: $path) {
            RecipeListView()
                .navigationDestination(for: NavigationDestination.self) { destination in
                    switch destination {
                    case .recipe(let id):
                        RecipeDetailView(recipeId: id)
                    case .settings:
                        SettingsView()
                    case .createRecipe:
                        RecipeFormView(mode: .create)
                    case .editRecipe(let id):
                        RecipeFormView(mode: .edit(recipeId: id))
                    }
                }
        }
        .onAppear { consumePendingRecipeId() }
        .onChange(of: appState.pendingRecipeId) { _ in consumePendingRecipeId() }
    }

    private func consumePendingRecipeId() {
        guard let id = appState.pendingRecipeId else { return }
        path.append(NavigationDestination.recipe(id))
        appState.pendingRecipeId = nil
    }
}

#Preview {
    ContentView()
        .environmentObject(AppState())
}
