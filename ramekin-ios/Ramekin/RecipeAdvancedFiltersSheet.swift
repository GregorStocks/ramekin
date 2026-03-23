import SwiftUI

struct RecipeAdvancedFiltersSheet: View {
    @Environment(\.dismiss) private var dismiss

    @Binding var source: String
    @Binding var createdAfter: String
    @Binding var createdBefore: String

    let onApply: () -> Void

    @State private var draftSource: String
    @State private var hasCreatedAfter: Bool
    @State private var draftCreatedAfter: Date
    @State private var hasCreatedBefore: Bool
    @State private var draftCreatedBefore: Date

    init(
        source: Binding<String>,
        createdAfter: Binding<String>,
        createdBefore: Binding<String>,
        onApply: @escaping () -> Void
    ) {
        _source = source
        _createdAfter = createdAfter
        _createdBefore = createdBefore
        self.onApply = onApply

        let normalizedSource = RecipeListFilterSupport.normalizedSource(source.wrappedValue)
        let normalizedCreatedAfter = RecipeListFilterSupport.normalizedDateValue(createdAfter.wrappedValue)
        let normalizedCreatedBefore = RecipeListFilterSupport.normalizedDateValue(createdBefore.wrappedValue)

        _draftSource = State(initialValue: normalizedSource)
        _hasCreatedAfter = State(initialValue: !normalizedCreatedAfter.isEmpty)
        _draftCreatedAfter = State(
            initialValue: RecipeListFilterSupport.date(from: normalizedCreatedAfter) ?? Date()
        )
        _hasCreatedBefore = State(initialValue: !normalizedCreatedBefore.isEmpty)
        _draftCreatedBefore = State(
            initialValue: RecipeListFilterSupport.date(from: normalizedCreatedBefore) ?? Date()
        )
    }

    private var hasInvalidDateRange: Bool {
        hasCreatedAfter && hasCreatedBefore && draftCreatedAfter > draftCreatedBefore
    }

    var body: some View {
        NavigationStack {
            Form {
                Section("Source") {
                    TextField("e.g. NYTimes", text: $draftSource)
                        .textInputAutocapitalization(.words)
                        .disableAutocorrection(true)
                }

                Section("Created") {
                    Toggle("Created after", isOn: $hasCreatedAfter.animation())

                    if hasCreatedAfter {
                        DatePicker(
                            "After",
                            selection: $draftCreatedAfter,
                            displayedComponents: .date
                        )
                    }

                    Toggle("Created before", isOn: $hasCreatedBefore.animation())

                    if hasCreatedBefore {
                        DatePicker(
                            "Before",
                            selection: $draftCreatedBefore,
                            displayedComponents: .date
                        )
                    }

                    if hasInvalidDateRange {
                        Text("The start date must be on or before the end date.")
                            .font(.caption)
                            .foregroundColor(.red)
                    }
                }

                Section {
                    Button("Clear Filters", role: .destructive) {
                        resetDraftFilters()
                    }
                }
            }
            .navigationTitle("Advanced Filters")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") {
                        dismiss()
                    }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Apply") {
                        applyFilters()
                    }
                    .disabled(hasInvalidDateRange)
                }
            }
        }
    }

    private func resetDraftFilters() {
        draftSource = ""
        hasCreatedAfter = false
        draftCreatedAfter = Date()
        hasCreatedBefore = false
        draftCreatedBefore = Date()
    }

    private func applyFilters() {
        guard !hasInvalidDateRange else {
            return
        }

        source = RecipeListFilterSupport.normalizedSource(draftSource)
        createdAfter = hasCreatedAfter
            ? RecipeListFilterSupport.dateOnlyString(from: draftCreatedAfter)
            : ""
        createdBefore = hasCreatedBefore
            ? RecipeListFilterSupport.dateOnlyString(from: draftCreatedBefore)
            : ""
        onApply()
        dismiss()
    }
}
