import SwiftUI

struct RecipeAdvancedFiltersSheet: View {
    @Environment(\.dismiss) private var dismiss

    @Binding var source: String
    @Binding var createdAfter: String
    @Binding var createdBefore: String
    @Binding var photoSizeFilter: String
    @Binding var photoDimensionFilter: String

    let onApply: () -> Void

    @State private var draftSource: String
    @State private var hasCreatedAfter: Bool
    @State private var draftCreatedAfter: Date
    @State private var hasCreatedBefore: Bool
    @State private var draftCreatedBefore: Date
    @State private var hasPhotoSizeFilter: Bool
    @State private var draftPhotoSizeOperator: NumericThresholdOperator
    @State private var draftPhotoSizeValue: String
    @State private var hasPhotoDimensionFilter: Bool
    @State private var draftPhotoDimensionOperator: NumericThresholdOperator
    @State private var draftPhotoDimensionValue: String

    init(
        source: Binding<String>,
        createdAfter: Binding<String>,
        createdBefore: Binding<String>,
        photoSizeFilter: Binding<String>,
        photoDimensionFilter: Binding<String>,
        onApply: @escaping () -> Void
    ) {
        _source = source
        _createdAfter = createdAfter
        _createdBefore = createdBefore
        _photoSizeFilter = photoSizeFilter
        _photoDimensionFilter = photoDimensionFilter
        self.onApply = onApply

        let normalizedSource = RecipeListFilterSupport.normalizedSource(source.wrappedValue)
        let normalizedCreatedAfter = RecipeListFilterSupport.normalizedDateValue(createdAfter.wrappedValue)
        let normalizedCreatedBefore = RecipeListFilterSupport.normalizedDateValue(createdBefore.wrappedValue)
        let initialPhotoSize = RecipeListFilterSupport.numericThreshold(from: photoSizeFilter.wrappedValue)
        let initialPhotoDimension = RecipeListFilterSupport.numericThreshold(from: photoDimensionFilter.wrappedValue)

        _draftSource = State(initialValue: normalizedSource)
        _hasCreatedAfter = State(initialValue: !normalizedCreatedAfter.isEmpty)
        _draftCreatedAfter = State(
            initialValue: RecipeListFilterSupport.date(from: normalizedCreatedAfter) ?? Date()
        )
        _hasCreatedBefore = State(initialValue: !normalizedCreatedBefore.isEmpty)
        _draftCreatedBefore = State(
            initialValue: RecipeListFilterSupport.date(from: normalizedCreatedBefore) ?? Date()
        )
        _hasPhotoSizeFilter = State(initialValue: initialPhotoSize != nil)
        _draftPhotoSizeOperator = State(initialValue: initialPhotoSize?.comparison ?? .lessThan)
        _draftPhotoSizeValue = State(initialValue: initialPhotoSize.map { String($0.value) } ?? "")
        _hasPhotoDimensionFilter = State(initialValue: initialPhotoDimension != nil)
        _draftPhotoDimensionOperator = State(initialValue: initialPhotoDimension?.comparison ?? .lessThan)
        _draftPhotoDimensionValue = State(initialValue: initialPhotoDimension.map { String($0.value) } ?? "")
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

                thresholdSection(
                    title: "Photo file size",
                    enabled: $hasPhotoSizeFilter,
                    comparison: $draftPhotoSizeOperator,
                    value: $draftPhotoSizeValue,
                    placeholder: "Bytes"
                )

                thresholdSection(
                    title: "Photo dimensions",
                    enabled: $hasPhotoDimensionFilter,
                    comparison: $draftPhotoDimensionOperator,
                    value: $draftPhotoDimensionValue,
                    placeholder: "Minimum side, px"
                )

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

    private func thresholdSection(
        title: String,
        enabled: Binding<Bool>,
        comparison: Binding<NumericThresholdOperator>,
        value: Binding<String>,
        placeholder: String
    ) -> some View {
        Section(title) {
            Toggle("Enabled", isOn: enabled.animation())

            if enabled.wrappedValue {
                Picker("Comparison", selection: comparison) {
                    ForEach(NumericThresholdOperator.allCases, id: \.self) { comparison in
                        Text(comparison.label).tag(comparison)
                    }
                }
                .pickerStyle(.segmented)

                TextField(placeholder, text: value)
                    .keyboardType(.numberPad)
            }
        }
    }

    private func resetDraftFilters() {
        draftSource = ""
        hasCreatedAfter = false
        draftCreatedAfter = Date()
        hasCreatedBefore = false
        draftCreatedBefore = Date()
        hasPhotoSizeFilter = false
        draftPhotoSizeOperator = .lessThan
        draftPhotoSizeValue = ""
        hasPhotoDimensionFilter = false
        draftPhotoDimensionOperator = .lessThan
        draftPhotoDimensionValue = ""
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
        photoSizeFilter = RecipeListFilterSupport.thresholdQueryValue(
            isEnabled: hasPhotoSizeFilter,
            comparison: draftPhotoSizeOperator,
            value: draftPhotoSizeValue
        )
        photoDimensionFilter = RecipeListFilterSupport.thresholdQueryValue(
            isEnabled: hasPhotoDimensionFilter,
            comparison: draftPhotoDimensionOperator,
            value: draftPhotoDimensionValue
        )
        onApply()
        dismiss()
    }
}
