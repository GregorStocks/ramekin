import XCTest
@testable import Ramekin

final class SyncApplySharedVectorTests: XCTestCase {
    private struct Row {
        var changeSeq: Int64
        var fields: [String: Any]
        var pending: SyncApplySupport.PendingWrite?
        var dirty: Set<String>
    }

    private struct Incoming {
        let id: String
        let changeSeq: Int64
        let fields: [String: Any]
    }

    func testSyncApplyMatchesSharedVectors() throws {
        let url = try XCTUnwrap(
            Bundle(for: Self.self).url(forResource: "sync-apply", withExtension: "json")
        )
        let root = try XCTUnwrap(
            JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any]
        )
        let cases = try XCTUnwrap(root["cases"] as? [[String: Any]])
        XCTAssertFalse(cases.isEmpty)

        for vector in cases {
            let name = try XCTUnwrap(vector["name"] as? String)
            var rows = try Self.rows(vector["cache"])
            let response = try XCTUnwrap(vector["response"] as? [String: Any])
            let changes = try XCTUnwrap(response["changes"] as? [[String: Any]])
            let incoming = try changes.map { change -> Incoming in
                Incoming(
                    id: try XCTUnwrap(change["id"] as? String),
                    changeSeq: try XCTUnwrap((change["change_seq"] as? NSNumber)?.int64Value),
                    fields: try XCTUnwrap(change["fields"] as? [String: Any])
                )
            }

            let actions = SyncApplySupport.plan(
                cached: rows.mapValues { SyncApplySupport.CachedState(changeSeq: $0.changeSeq, pending: $0.pending) },
                changes: incoming.map { (id: $0.id, changeSeq: $0.changeSeq) },
                deleted: try XCTUnwrap(response["deleted"] as? [String]),
                fullResync: try XCTUnwrap(response["full_resync"] as? Bool)
            )
            for (id, action) in actions {
                switch action {
                case .keep:
                    break
                case .drop:
                    rows[id] = nil
                case .take:
                    let change = try XCTUnwrap(incoming.first { $0.id == id })
                    var row = rows[id] ?? Row(changeSeq: 0, fields: [:], pending: nil, dirty: [])
                    for (field, value) in change.fields where !row.dirty.contains(field) {
                        row.fields[field] = value
                    }
                    row.changeSeq = change.changeSeq
                    rows[id] = row
                }
            }

            let expected = try Self.rows(vector["expected"])
            XCTAssertEqual(Set(rows.keys), Set(expected.keys), name)
            for (id, row) in expected {
                let actual = try XCTUnwrap(rows[id], name)
                XCTAssertEqual(actual.changeSeq, row.changeSeq, "\(name): \(id)")
                XCTAssertEqual(actual.pending, row.pending, "\(name): \(id)")
                XCTAssertEqual(actual.dirty, row.dirty, "\(name): \(id)")
                XCTAssertEqual(
                    NSDictionary(dictionary: actual.fields),
                    NSDictionary(dictionary: row.fields),
                    "\(name): \(id)"
                )
            }
        }
    }

    private static func rows(_ value: Any?) throws -> [String: Row] {
        let list = try XCTUnwrap(value as? [[String: Any]])
        var rows: [String: Row] = [:]
        for row in list {
            let pending = row["pending"] as? String
            rows[try XCTUnwrap(row["id"] as? String)] = Row(
                changeSeq: try XCTUnwrap((row["change_seq"] as? NSNumber)?.int64Value),
                fields: try XCTUnwrap(row["fields"] as? [String: Any]),
                pending: try pending.map { try XCTUnwrap(SyncApplySupport.PendingWrite(rawValue: $0)) },
                dirty: Set(try XCTUnwrap(row["dirty"] as? [String]))
            )
        }
        return rows
    }
}
