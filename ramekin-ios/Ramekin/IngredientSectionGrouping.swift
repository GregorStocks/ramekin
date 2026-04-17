import Foundation

func groupConsecutiveItemsBySection<Item>(
    _ items: [Item],
    section: (Item) -> String?
) -> [(section: String?, items: [Item])] {
    var groups: [(section: String?, items: [Item])] = []
    var currentSection: String?
    var currentItems: [Item] = []

    for item in items {
        let itemSection = section(item)
        if itemSection != currentSection {
            if !currentItems.isEmpty {
                groups.append((section: currentSection, items: currentItems))
            }
            currentSection = itemSection
            currentItems = [item]
        } else {
            currentItems.append(item)
        }
    }

    if !currentItems.isEmpty {
        groups.append((section: currentSection, items: currentItems))
    }

    return groups
}
