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

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(appState)
        }
    }
}

/// Global app state shared across views
class AppState: ObservableObject {
    @Published var isLoggedIn: Bool = false
    @Published var username: String = ""
    @Published var serverURL: String = ""
    @Published var pendingRecipeId: UUID?
    private var lastHandledUniversalLink: URL?
    private var lastHandledUniversalLinkAt = Date.distantPast
    private var universalLinkObserver: NSObjectProtocol?

    init() {
        refreshState()
        universalLinkObserver = NotificationCenter.default.addObserver(
            forName: .universalLinkReceived,
            object: nil,
            queue: .main
        ) { [weak self] notification in
            guard let url = notification.object as? URL else { return }
            self?.handleUniversalLink(url)
        }
        if let url = UniversalLinkStore.shared.consumePendingURL() {
            handleUniversalLink(url)
        }
    }

    deinit {
        if let universalLinkObserver {
            NotificationCenter.default.removeObserver(universalLinkObserver)
        }
    }

    func refreshState() {
        isLoggedIn = RamekinAPI.shared.isLoggedIn
        username = KeychainHelper.shared.getUsername() ?? ""
        serverURL = KeychainHelper.shared.getServerURL() ?? ""
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
