import SearchCore
import SwiftUI

/// アプリ側で持つ「本体DB」を模した最小レコード。
/// 本物のアプリでは SwiftData/Core Data のエンティティに相当する位置づけ。
struct Record: Identifiable, Hashable {
    let id: Int64
    let text: String
}

@MainActor
final class SearchModel: ObservableObject {
    @Published var query: String = ""
    @Published var status: String = ""
    @Published var results: [Record] = []

    private let engine: SearchEngine
    /// 検索エンジンは ID と score しか返さないため、本体側で id→Record の引き直しを行う。
    /// 設計書 §1.3「IDのみ返却 / 本体DBから再フェッチ」をミニチュア実装したもの。
    private var store: [Int64: Record] = [:]

    init() {
        let url = FileManager.default
            .urls(for: .documentDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("search_index.sqlite")
        do {
            self.engine = try SearchEngine(dbPath: url.path)
        } catch {
            fatalError("open SearchEngine failed: \(error)")
        }
        seedIfNeeded()
    }

    private func seedIfNeeded() {
        let seed: [Record] = [
            Record(id: 1, text: "東京タワー"),
            Record(id: 2, text: "とうきょうスカイツリー"),
            Record(id: 3, text: "ﾄｳｷｮｳ ﾄﾞｰﾑ"),
            Record(id: 4, text: "Osaka 城"),
            Record(id: 5, text: "がっこう ぐらし"),
            Record(id: 6, text: "かっこう の歌"),
            Record(id: 7, text: "Ｐｙｔｈｏｎ 入門"),
            Record(id: 8, text: "ぱんだ と ﾊﾟﾝﾀﾞ")
        ]
        for record in seed {
            try? engine.index(id: record.id, text: record.text)
            store[record.id] = record
        }
        status = "indexed \(seed.count) docs"
        if let auto = ProcessInfo.processInfo.environment["SEARCH_AUTO_QUERY"] {
            query = auto
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { [weak self] in
                self?.search()
            }
        }
    }

    func search() {
        do {
            let hits = try engine.search(query: query, limit: 50)
            // ID で本体ストアから引き直す(失われたレコードはスキップ)。
            results = hits.compactMap { store[$0.id] }
            status = "hits: \(results.count)  normalized=\u{0022}\(normalizeLoose(input: query))\u{0022}"
        } catch {
            status = "error: \(error)"
            results = []
        }
    }
}

struct ContentView: View {
    @StateObject private var model = SearchModel()

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 12) {
                TextField("検索クエリ(全角/半角/カナ/ひら、なんでも)", text: $model.query)
                    .textFieldStyle(.roundedBorder)
                    .onSubmit { model.search() }
                Button("検索") { model.search() }
                    .buttonStyle(.borderedProminent)
                Text(model.status)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                List(model.results) { record in
                    VStack(alignment: .leading, spacing: 4) {
                        Text(record.text)
                            .font(.body)
                        Text("id=\(record.id)")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .monospacedDigit()
                    }
                    .padding(.vertical, 2)
                }
            }
            .padding()
            .navigationTitle("SearchSample")
        }
    }
}
