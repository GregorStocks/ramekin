import XCTest
@testable import Ramekin

final class TagHierarchySharedVectorTests: XCTestCase {
    private struct Vectors: Decodable {
        let seededNamespaces: [String]
        let parseCases: [ParseCase]
        let formatCases: [FormatCase]
        let normalizeNamespaceCases: [NormalizeCase]
        let groupCases: [GroupCase]
        let knownNamespacesCases: [KnownNamespacesCase]
    }

    private struct ParseCase: Decodable {
        let name: String
        let input: String
        let namespace: String?
        let value: String
    }

    private struct FormatCase: Decodable {
        let name: String
        let namespace: String?
        let value: String
        let expected: String
    }

    private struct NormalizeCase: Decodable {
        let name: String
        let input: String
        let expected: String?
    }

    private struct GroupCase: Decodable {
        let name: String
        let names: [String]
        let expected: [ExpectedGroup]
    }

    private struct ExpectedGroup: Decodable, Equatable {
        let namespace: String?
        let names: [String]
    }

    private struct KnownNamespacesCase: Decodable {
        let name: String
        let names: [String]
        let expected: [String]
    }

    func testTagHierarchyMatchesSharedVectors() throws {
        let vectors = try loadVectors()

        XCTAssertEqual(TagHierarchySupport.seededNamespaces, vectors.seededNamespaces)

        for vector in vectors.parseCases {
            let parsed = TagHierarchySupport.parse(name: vector.input)
            XCTAssertEqual(parsed.namespace, vector.namespace, vector.name)
            XCTAssertEqual(parsed.value, vector.value, vector.name)
        }

        for vector in vectors.formatCases {
            XCTAssertEqual(
                TagHierarchySupport.format(namespace: vector.namespace, value: vector.value),
                vector.expected,
                vector.name
            )
        }

        for vector in vectors.normalizeNamespaceCases {
            XCTAssertEqual(
                TagHierarchySupport.normalizedNamespace(from: vector.input),
                vector.expected,
                vector.name
            )
        }

        for vector in vectors.groupCases {
            let actual = TagHierarchySupport.groups(for: vector.names).map {
                ExpectedGroup(namespace: $0.namespace, names: $0.items.map(\.name))
            }
            XCTAssertEqual(actual, vector.expected, vector.name)
        }

        for vector in vectors.knownNamespacesCases {
            XCTAssertEqual(
                TagHierarchySupport.availableNamespaces(from: vector.names),
                vector.expected,
                vector.name
            )
        }
    }

    private func loadVectors() throws -> Vectors {
        let url = try XCTUnwrap(
            Bundle(for: Self.self).url(forResource: "tag-hierarchy", withExtension: "json")
        )
        return try JSONDecoder().decode(Vectors.self, from: Data(contentsOf: url))
    }
}
