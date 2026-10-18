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
    /// Plans live in the offline cache, so any week shows at once; syncing
    /// only refreshes it.
    @ObservedObject private var store = MealPlanStore.shared
    @State private var weekStart: Date = MealPlanDateSupport.mondayStart(from: Date())
    /// Failures of add/edit, shown as an alert.
    @State private var actionError: String?

    @State private var showingRecipePicker = false
    @State private var pickerDate: Date = Date()
    @State private var pickerMealType: MealType = .dinner

    @State private var deletingMealPlan: MealPlanItem?
    @State private var editingMealPlan: MealPlanItem?
    /// Day to scroll to once an add or edit has reloaded the week. Closing
    /// the picker sheet can leave the week scrolled back to Monday, so the
    /// edited day is brought back into view explicitly.
    @State private var scrollTarget: Date?

    private let logger = DebugLogger.shared

    var body: some View {
        NavigationStack {
            weekCalendar
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
                await store.syncWithServer()
                logger.log("Pull-to-refresh completed", source: "MealPlan")
            }
            .task {
                await store.syncWithServer()
            }
            .onReceive(NotificationCenter.default.publisher(for: .recipeDeleted)) { _ in
                // The server tombstones a deleted recipe's plans.
                Task { await store.syncWithServer() }
            }
            .sheet(isPresented: $showingRecipePicker) {
                RecipePickerSheet(
                    date: pickerDate,
                    mealType: pickerMealType
                ) { recipe in
                    // Another Add sheet can reassign the picker state before
                    // this add finishes, so pass the slot along with it.
                    addMealPlan(recipe: recipe, date: pickerDate, mealType: pickerMealType)
                }
            }
            .sheet(item: $editingMealPlan) { meal in
                EditMealPlanSheet(meal: meal) { mealDate, mealType, notes in
                    try updateMealPlan(
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
                        store.delete(meal)
                        deletingMealPlan = nil
                    }
                    Button("Cancel", role: .cancel) {
                        deletingMealPlan = nil
                    }
                }
            }
            .alert(
                "Couldn't Update Meal Plan",
                isPresented: Binding(
                    get: { actionError != nil || store.refusal != nil },
                    set: {
                        if !$0 {
                            actionError = nil
                            store.refusal = nil
                        }
                    }
                )
            ) {
                Button("OK", role: .cancel) {}
            } message: {
                Text(actionError ?? store.refusal ?? "")
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
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 0) {
                    weekHeader
                    ForEach(daysInWeek, id: \.self) { date in
                        daySection(date: date)
                            .id(date)
                    }
                }
            }
            .onChange(of: scrollTarget) { target in
                guard let target else { return }
                withAnimation {
                    proxy.scrollTo(target, anchor: .top)
                }
                scrollTarget = nil
            }
        }
    }

    private var weekHeader: some View {
        let endDate = Calendar.current.date(byAdding: .day, value: 6, to: weekStart)!
        let start = Self.weekRangeFormatter.string(from: weekStart)
        let end = Self.weekRangeFormatter.string(from: endDate)
        return VStack(spacing: 2) {
            Text("\(start) – \(end)")
                .font(.subheadline)
                .foregroundColor(.secondary)
            if store.lastSyncError != nil {
                Text("Showing saved meals. Pull down to sync.")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
        }
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

    // MARK: - Helpers

    private func mealsFor(date: Date, mealType: MealType) -> [MealPlanItem] {
        store.plans(on: date, mealType: mealType)
    }

}

// MARK: - Editing

extension MealPlanView {
    private func addMealPlan(recipe: RecipeSummary, date: Date, mealType: MealType) {
        do {
            try store.add(
                recipeId: recipe.id,
                recipeTitle: recipe.title,
                thumbnailPhotoId: recipe.thumbnailPhotoId,
                date: date,
                mealType: mealType
            )
            scrollTarget = Calendar.current.startOfDay(for: date)
        } catch {
            logger.log("addMealPlan error: \(error.localizedDescription)", source: "MealPlan")
            actionError = error.localizedDescription
        }
    }

    private func updateMealPlan(
        _ meal: MealPlanItem,
        mealDate: Date,
        mealType: MealType,
        notes: String
    ) throws {
        try store.update(meal, date: mealDate, mealType: mealType, notes: notes)
        // A meal moved to another day would otherwise end up off screen.
        let oldDay = Calendar.current.startOfDay(
            for: MealPlanDateSupport.localDate(fromAPIDate: meal.mealDate)
        )
        let newDay = Calendar.current.startOfDay(for: mealDate)
        if newDay != oldDay {
            scrollTarget = newDay
        }
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
