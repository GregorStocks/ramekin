import SwiftUI

// MARK: - MealType Helpers

extension MealType {
    var displayLabel: String {
        switch self {
        case .breakfast: return "Breakfast"
        case .lunch: return "Lunch"
        case .dinner: return "Dinner"
        case .snack: return "Snack"
        }
    }

    static var displayOrder: [MealType] {
        [.breakfast, .lunch, .dinner, .snack]
    }
}

// MARK: - MealPlanView

struct MealPlanView: View {
    @State private var weekStart: Date = MealPlanDateSupport.mondayStart(from: Date())
    @State private var mealPlans: [MealPlanItem] = []
    @State private var isLoading = false
    // The week whose meal plans are loaded. Reloading the same week after an
    // add or edit updates it in place (keeping the scroll position, even when
    // the week has no meals yet); switching weeks shows the loading state.
    @State private var loadedWeek: Date?
    @State private var error: String?
    /// Failures of add/remove once the week is on screen; shown as an alert
    /// because `error` only replaces a week that hasn't loaded.
    @State private var actionError: String?

    @State private var showingRecipePicker = false
    @State private var pickerDate: Date = Date()
    @State private var pickerMealType: MealType = .dinner

    @State private var deletingMealPlan: MealPlanItem?
    @State private var editingMealPlan: MealPlanItem?

    private let logger = DebugLogger.shared

    var body: some View {
        NavigationStack {
            Group {
                if isLoading && loadedWeek != weekStart {
                    ProgressView("Loading meal plans…")
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else if let error = error, loadedWeek != weekStart {
                    errorView(message: error)
                } else {
                    weekCalendar
                }
            }
            .navigationTitle("Meal Plan")
            .toolbar {
                ToolbarItemGroup(placement: .bottomBar) {
                    Button {
                        weekStart = Calendar.current.date(byAdding: .day, value: -7, to: weekStart)!
                    } label: {
                        Image(systemName: "chevron.left")
                    }
                    .accessibilityLabel("Previous Week")

                    Spacer()

                    Button("Today") {
                        weekStart = MealPlanDateSupport.mondayStart(from: Date())
                    }

                    Spacer()

                    Button {
                        weekStart = Calendar.current.date(byAdding: .day, value: 7, to: weekStart)!
                    } label: {
                        Image(systemName: "chevron.right")
                    }
                    .accessibilityLabel("Next Week")
                }
            }
            .refreshable {
                logger.log("Pull-to-refresh started", source: "MealPlan")
                await loadMealPlans()
                logger.log("Pull-to-refresh completed", source: "MealPlan")
            }
            .task {
                await loadMealPlans()
            }
            .onChange(of: weekStart) { _ in
                // The old week's meals don't belong to the new dates; clearing
                // them shows the loading state instead of an empty week.
                mealPlans = []
                loadedWeek = nil
                Task { await loadMealPlans() }
            }
            .onReceive(NotificationCenter.default.publisher(for: .recipeDeleted)) { _ in
                Task { await loadMealPlans() }
            }
            .sheet(isPresented: $showingRecipePicker) {
                RecipePickerSheet(
                    date: pickerDate,
                    mealType: pickerMealType
                ) { recipe in
                    Task { await addMealPlan(recipe: recipe) }
                }
            }
            .sheet(item: $editingMealPlan) { meal in
                EditMealPlanSheet(meal: meal) { mealDate, mealType, notes in
                    try await updateMealPlan(
                        meal,
                        mealDate: mealDate,
                        mealType: mealType,
                        notes: notes
                    )
                }
            }
            .confirmationDialog(
                "Remove from meal plan?",
                isPresented: Binding(
                    get: { deletingMealPlan != nil },
                    set: { if !$0 { deletingMealPlan = nil } }
                ),
                titleVisibility: .visible
            ) {
                if let meal = deletingMealPlan {
                    Button("Remove \(meal.recipeTitle)", role: .destructive) {
                        Task { await deleteMealPlan(meal) }
                    }
                    Button("Cancel", role: .cancel) {
                        deletingMealPlan = nil
                    }
                }
            }
            .alert(
                "Couldn't Update Meal Plan",
                isPresented: Binding(
                    get: { actionError != nil },
                    set: { if !$0 { actionError = nil } }
                )
            ) {
                Button("OK", role: .cancel) {}
            } message: {
                Text(actionError ?? "")
            }
            .navigationDestination(for: NavigationDestination.self) { destination in
                switch destination {
                case .recipe(let id):
                    RecipeDetailView(recipeId: id)
                case .settings:
                    SettingsView()
                }
            }
        }
    }

    // MARK: - Week Calendar

    private var weekCalendar: some View {
        ScrollView {
            LazyVStack(spacing: 0) {
                weekHeader
                ForEach(daysInWeek, id: \.self) { date in
                    daySection(date: date)
                }
            }
        }
    }

    private var weekHeader: some View {
        let endDate = Calendar.current.date(byAdding: .day, value: 6, to: weekStart)!
        let start = Self.weekRangeFormatter.string(from: weekStart)
        let end = Self.weekRangeFormatter.string(from: endDate)
        return Text("\(start) – \(end)")
            .font(.subheadline)
            .foregroundColor(.secondary)
            .frame(maxWidth: .infinity)
            .padding(.vertical, 8)
    }

    private var daysInWeek: [Date] {
        (0..<7).compactMap { offset in
            Calendar.current.date(byAdding: .day, value: offset, to: weekStart)
        }
    }

    private func daySection(date: Date) -> some View {
        let isToday = Calendar.current.isDateInToday(date)
        return VStack(alignment: .leading, spacing: 0) {
            // Day header
            HStack {
                Text(dayHeaderText(date))
                    .font(.headline)
                    // Black on orange; white on orange is ~2:1 contrast.
                    .foregroundColor(isToday ? .black : .primary)
                    .accessibilityAddTraits(.isHeader)
                Spacer()
            }
            .padding(.horizontal)
            .padding(.vertical, 10)
            .background(isToday ? Color.orange : Color(.systemGray6))

            // Meal type sections
            VStack(alignment: .leading, spacing: 0) {
                ForEach(MealType.displayOrder, id: \.self) { mealType in
                    mealSlot(date: date, mealType: mealType)
                }
            }
            .padding(.horizontal)
            .padding(.vertical, 8)

            Divider()
        }
    }

    private func dayHeaderText(_ date: Date) -> String {
        Self.dayHeaderFormatter.string(from: date)
    }

    private func mealSlot(date: Date, mealType: MealType) -> some View {
        let meals = mealsFor(date: date, mealType: mealType)
        return VStack(alignment: .leading, spacing: 4) {
            Text(mealType.displayLabel)
                .font(.subheadline)
                .fontWeight(.medium)
                .foregroundColor(.secondary)
                .padding(.top, 6)

            ForEach(meals) { meal in
                MealPlanItemCard(
                    meal: meal,
                    onEdit: { editingMealPlan = meal },
                    onDelete: { deletingMealPlan = meal }
                )
            }

            Button {
                pickerDate = date
                pickerMealType = mealType
                showingRecipePicker = true
            } label: {
                Label("Add", systemImage: "plus.circle")
                    .font(.caption)
                    .foregroundColor(.orange)
                    .frame(minHeight: 44)
                    .contentShape(Rectangle())
            }
            .accessibilityLabel("Add \(mealType.displayLabel) on \(dayHeaderText(date))")
        }
    }

    // MARK: - Error View

    private func errorView(message: String) -> some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle")
                .accessibilityHidden(true)
                .font(.largeTitle)
                .foregroundColor(.orange)
            Text(message)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal)
            Button("Retry") {
                Task { await loadMealPlans() }
            }
            .buttonStyle(.borderedProminent)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: - Helpers

    private func mealsFor(date: Date, mealType: MealType) -> [MealPlanItem] {
        let dateString = Self.localDateString(from: date)
        return mealPlans.filter { meal in
            let mealDate = MealPlanDateSupport.localDate(fromAPIDate: meal.mealDate)
            return Self.localDateString(from: mealDate) == dateString && meal.mealType == mealType
        }
    }

    private static func localDateString(from date: Date) -> String {
        SharedDateFormatters.localDateOnly.string(from: date)
    }

}

// MARK: - Data Loading

extension MealPlanView {
    private func loadMealPlans() async {
        await MainActor.run {
            isLoading = true
            error = nil
        }

        let requestedWeek = weekStart
        let endDate = Calendar.current.date(byAdding: .day, value: 6, to: requestedWeek)!

        do {
            let response = try await logger.timed("listMealPlans API", source: "MealPlan") {
                try await RamekinAPI.shared.listMealPlans(startDate: requestedWeek, endDate: endDate)
            }
            await MainActor.run {
                // A slow response for a week the user has left must not
                // replace the week on screen.
                guard weekStart == requestedWeek else { return }
                mealPlans = response.mealPlans
                loadedWeek = requestedWeek
                isLoading = false
            }
        } catch is CancellationError {
            logger.log("loadMealPlans cancelled", source: "MealPlan")
        } catch {
            await MainActor.run {
                guard weekStart == requestedWeek else { return }
                if loadedWeek != requestedWeek {
                    self.error = "Could not load meal plans. Please try again."
                } else {
                    actionError = "Couldn't refresh this week. Pull down to try again."
                }
                isLoading = false
            }
        }
    }

    private func addMealPlan(recipe: RecipeSummary) async {
        do {
            _ = try await logger.timed("createMealPlan API", source: "MealPlan") {
                try await RamekinAPI.shared.createMealPlan(
                    recipeId: recipe.id,
                    mealDate: pickerDate,
                    mealType: pickerMealType.rawValue
                )
            }
            await loadMealPlans()
        } catch is CancellationError {
            // ignored
        } catch {
            logger.log("addMealPlan error: \(error.localizedDescription)", source: "MealPlan")
            let message = APIErrorFormatter.code(from: error) == .conflict
                ? "This recipe is already planned for this meal."
                : APIErrorFormatter.userMessage(from: error, fallback: "Failed to add to meal plan")
            await MainActor.run {
                actionError = message
            }
        }
    }

    private func deleteMealPlan(_ meal: MealPlanItem) async {
        do {
            try await logger.timed("deleteMealPlan API", source: "MealPlan") {
                try await RamekinAPI.shared.deleteMealPlan(id: meal.id)
            }
            await loadMealPlans()
        } catch is CancellationError {
            // ignored
        } catch {
            logger.log("deleteMealPlan error: \(error.localizedDescription)", source: "MealPlan")
            await MainActor.run {
                actionError = APIErrorFormatter.userMessage(
                    from: error, fallback: "Failed to remove from meal plan"
                )
            }
        }
    }

    private func updateMealPlan(
        _ meal: MealPlanItem,
        mealDate: Date,
        mealType: MealType,
        notes: String
    ) async throws {
        try await logger.timed("updateMealPlan API", source: "MealPlan") {
            try await RamekinAPI.shared.updateMealPlan(
                id: meal.id,
                mealDate: mealDate,
                mealType: mealType.rawValue,
                notes: notes
            )
        }
        await loadMealPlans()
    }
}

extension MealPlanView {
    static let weekRangeFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateFormat = "MMM d"
        formatter.calendar = .autoupdatingCurrent
        formatter.locale = .autoupdatingCurrent
        formatter.timeZone = .autoupdatingCurrent
        return formatter
    }()

    static let dayHeaderFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateFormat = "EEEE, MMM d"
        formatter.calendar = .autoupdatingCurrent
        formatter.locale = .autoupdatingCurrent
        formatter.timeZone = .autoupdatingCurrent
        return formatter
    }()
}

#Preview {
    MealPlanView()
}
