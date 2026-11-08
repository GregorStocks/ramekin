import SwiftUI

struct TagManagementView: View {
    @StateObject private var model = TagManagementViewModel()

    var body: some View {
        List {
            if let error = model.loadError {
                Section {
                    Text(error)
                        .foregroundColor(.red)
                }
            }

            if model.isLoading && model.tags.isEmpty {
                Section {
                    HStack {
                        Spacer()
                        ProgressView("Loading tags…")
                        Spacer()
                    }
                    .padding(.vertical, 12)
                }
            } else if model.tags.isEmpty {
                Section {
                    VStack(spacing: 12) {
                        Image(systemName: "tag.slash")
                            .scaledIconFont(size: 32)
                            .accessibilityHidden(true)
                            .foregroundColor(.secondary)
                        Text("No tags yet")
                            .font(.headline)
                        Text("Tags are created when you add them to recipes.")
                            .font(.subheadline)
                            .foregroundColor(.secondary)
                            .multilineTextAlignment(.center)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 24)
                }
            } else {
                ForEach(model.groupedTags) { group in
                    Section(group.title) {
                        ForEach(group.items) { tag in
                            row(for: tag)
                        }
                    }
                }

                Section {
                    EmptyView()
                } footer: {
                    Text("Deleting a tag removes it from recipes that currently use it.")
                }
            }
        }
        .listStyle(.insetGrouped)
        .navigationTitle("Manage Tags")
        .navigationBarTitleDisplayMode(.inline)
        .refreshable {
            await model.loadTags()
        }
        .task {
            await model.loadTags()
        }
        .alert(
            "Delete Tag",
            isPresented: Binding(
                get: { model.deleteCandidate != nil },
                set: { if !$0 { model.deleteCandidate = nil } }
            ),
            presenting: model.deleteCandidate,
            actions: { tag in
                Button("Delete", role: .destructive) {
                    Task { await model.delete(tag) }
                }
                Button("Cancel", role: .cancel) {}
            },
            message: { tag in
                Text("Remove \"\(tag.name)\" from \(TagManagementSupport.recipeCountText(for: tag.recipeCount))?")
            }
        )
        .alert(
            "Couldn't Delete Tag",
            isPresented: Binding(
                get: { model.deleteError != nil },
                set: { if !$0 { model.deleteError = nil } }
            ),
            actions: { Button("OK", role: .cancel) {} },
            message: { Text(model.deleteError ?? "") }
        )
    }
}

private extension TagManagementView {
    @ViewBuilder
    func row(for tag: TagItem) -> some View {
        if model.editingTagId == tag.id {
            editingRow(for: tag)
        } else {
            tagRow(for: tag)
        }
    }

    func tagRow(for tag: TagItem) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .top, spacing: 12) {
                VStack(alignment: .leading, spacing: 4) {
                    HierarchicalTagLabel(name: tag.name, valueFont: .headline, namespaceFont: .subheadline)
                    Text(TagManagementSupport.recipeCountText(for: tag.recipeCount))
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                }

                Spacer()

                if model.isDeleting(tag) {
                    ProgressView()
                        .accessibilityLabel("Deleting")
                }
            }

            HStack(spacing: 12) {
                Button {
                    model.startEditing(tag)
                } label: {
                    Label("Rename", systemImage: "pencil")
                }
                .buttonStyle(.borderless)

                Button(role: .destructive) {
                    model.deleteCandidate = tag
                } label: {
                    Label("Delete", systemImage: "trash")
                }
                .buttonStyle(.borderless)
            }
            .font(.subheadline)
            .disabled(model.isDeleting(tag))
        }
        .padding(.vertical, 6)
    }

    func editingRow(for tag: TagItem) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            TextField("Tag name", text: $model.editName)
                .textInputAutocapitalization(.never)
                .disableAutocorrection(true)
                .submitLabel(.done)
                .onSubmit {
                    Task { await model.saveEdit() }
                }

            Text(TagManagementSupport.recipeCountText(for: tag.recipeCount))
                .font(.subheadline)
                .foregroundColor(.secondary)

            if let editError = model.editError {
                Text(editError)
                    .font(.caption)
                    .foregroundColor(.red)
            }

            HStack(spacing: 12) {
                Button {
                    Task { await model.saveEdit() }
                } label: {
                    if model.isSavingEdit {
                        ProgressView()
                    } else {
                        Text("Save")
                    }
                }
                .buttonStyle(.borderedProminent)
                // One rename at a time, even when it belongs to another tag.
                .disabled(model.savingTagId != nil)

                Button("Cancel") {
                    model.cancelEditing()
                }
                .buttonStyle(.bordered)
                .disabled(model.isSavingEdit)
            }
        }
        .padding(.vertical, 6)
    }
}

#Preview {
    NavigationStack {
        TagManagementView()
    }
}
