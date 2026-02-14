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
    @State private var weekStart: Date = getMonday(from: Date())
    @State private var mealPlans: [MealPlanItem] = []
    @State private var isLoading = false
    @State private var error: String?

    @State private var showingRecipePicker = false
    @State private var pickerDate: Date = Date()
    @State private var pickerMealType: MealType = .dinner

    @State private var deletingMealPlan: MealPlanItem?

    private let logger = DebugLogger.shared

    var body: some View {
        NavigationStack {
            Group {
                if isLoading && mealPlans.isEmpty {
                    ProgressView("Loading meal plans...")
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else if let error = error, mealPlans.isEmpty {
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

                    Spacer()

                    Button("Today") {
                        weekStart = Self.getMonday(from: Date())
                    }

                    Spacer()

                    Button {
                        weekStart = Calendar.current.date(byAdding: .day, value: 7, to: weekStart)!
                    } label: {
                        Image(systemName: "chevron.right")
                    }
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
        let formatter = DateFormatter()
        formatter.dateFormat = "MMM d"
        let start = formatter.string(from: weekStart)
        let end = formatter.string(from: endDate)
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
                    .foregroundColor(isToday ? .white : .primary)
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
        let formatter = DateFormatter()
        formatter.dateFormat = "EEEE, MMM d"
        return formatter.string(from: date)
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
                NavigationLink(value: NavigationDestination.recipe(meal.recipeId)) {
                    mealCard(meal)
                }
                .buttonStyle(.plain)
            }

            Button {
                pickerDate = date
                pickerMealType = mealType
                showingRecipePicker = true
            } label: {
                Label("Add", systemImage: "plus.circle")
                    .font(.caption)
                    .foregroundColor(.orange)
            }
            .padding(.bottom, 4)
        }
    }

    private func mealCard(_ meal: MealPlanItem) -> some View {
        HStack(spacing: 10) {
            RecipeThumbnail(photoId: meal.thumbnailPhotoId, size: 44)

            Text(meal.recipeTitle)
                .font(.subheadline)
                .foregroundColor(.primary)
                .lineLimit(2)

            Spacer()

            Button {
                deletingMealPlan = meal
            } label: {
                Image(systemName: "xmark.circle.fill")
                    .foregroundColor(.secondary)
                    .font(.body)
            }
            .buttonStyle(.plain)
        }
        .padding(.vertical, 4)
        .padding(.horizontal, 8)
        .background(Color(.systemGray6))
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }

    // MARK: - Error View

    private func errorView(message: String) -> some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle")
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
            Self.localDateString(from: meal.mealDate) == dateString && meal.mealType == mealType
        }
    }

    private static func localDateString(from date: Date) -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd"
        return formatter.string(from: date)
    }

    static func getMonday(from date: Date) -> Date {
        let calendar = Calendar.current
        let weekday = calendar.component(.weekday, from: date)
        // weekday: 1=Sun, 2=Mon, ..., 7=Sat
        let daysToSubtract = (weekday + 5) % 7
        return calendar.date(byAdding: .day, value: -daysToSubtract, to: calendar.startOfDay(for: date))!
    }

    // MARK: - Data Loading

    private func loadMealPlans() async {
        await MainActor.run {
            isLoading = true
            error = nil
        }

        let endDate = Calendar.current.date(byAdding: .day, value: 6, to: weekStart)!

        do {
            let response = try await logger.timed("listMealPlans API", source: "MealPlan") {
                try await RamekinAPI.shared.listMealPlans(startDate: weekStart, endDate: endDate)
            }
            await MainActor.run {
                mealPlans = response.mealPlans
                isLoading = false
            }
        } catch is CancellationError {
            logger.log("loadMealPlans cancelled", source: "MealPlan")
        } catch {
            await MainActor.run {
                if mealPlans.isEmpty {
                    self.error = "Could not load meal plans. Please try again."
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
            await MainActor.run {
                self.error = error.localizedDescription
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
                self.error = error.localizedDescription
            }
        }
    }
}

#Preview {
    MealPlanView()
}
