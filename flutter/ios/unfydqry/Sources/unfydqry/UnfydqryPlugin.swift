import Flutter
import UIKit
// The generated UniFFI binding (SearchEngine, NormalizeOptions, …) is compiled
// into this same module from UnifiedQueryBinding.swift, so no import is needed.

/// iOS side of the Flutter plugin.
///
/// Each open engine lives in ``engines`` keyed by an integer handle that is
/// returned to Dart on "open" and sent back on every subsequent call.
///
/// Threading: the Flutter method channel invokes ``handle(_:result:)`` on the
/// platform main thread, so ``engines`` and ``nextHandle`` need no locking.
///
/// Lifetime: unlike the Android side there is no explicit `close()` — the
/// UniFFI-generated `SearchEngine` frees its Rust pointer in `deinit`, so
/// dropping the last reference (via `removeValue`) releases it deterministically
/// under ARC. That is the only intended asymmetry with `UnfydqryPlugin.kt`.
public class UnfydqryPlugin: NSObject, FlutterPlugin {

    private var engines: [Int: SearchEngine] = [:]
    private var nextHandle = 0

    public static func register(with registrar: FlutterPluginRegistrar) {
        let channel = FlutterMethodChannel(
            name: "unfydqry/search",
            binaryMessenger: registrar.messenger()
        )
        let instance = UnfydqryPlugin()
        registrar.addMethodCallDelegate(instance, channel: channel)
    }

    public func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
        let args = call.arguments as? [String: Any] ?? [:]
        do {
            switch call.method {

            case "open":
                guard let dbPath = args["dbPath"] as? String else {
                    return result(badArgs("dbPath:String required"))
                }
                let engine = try SearchEngine(dbPath: dbPath)
                let handle = nextHandle
                nextHandle += 1
                engines[handle] = engine
                result(handle)

            case "openWithOptions":
                guard let dbPath = args["dbPath"] as? String else {
                    return result(badArgs("dbPath:String required"))
                }
                guard let config = engineConfig(args, result: result) else { return }
                let engine = try SearchEngine.withOptions(dbPath: dbPath, config: config)
                let handle = nextHandle
                nextHandle += 1
                engines[handle] = engine
                result(handle)

            case "openWithOptionsRebuilding":
                guard let dbPath = args["dbPath"] as? String else {
                    return result(badArgs("dbPath:String required"))
                }
                guard let config = engineConfig(args, result: result) else { return }
                let engine = try SearchEngine.withOptionsRebuilding(dbPath: dbPath, config: config)
                let handle = nextHandle
                nextHandle += 1
                engines[handle] = engine
                result(handle)

            case "normalizeWithOptions":
                guard let input = args["input"] as? String else {
                    return result(badArgs("input:String required"))
                }
                guard let options = normalizeOptions(args, result: result) else { return }
                result(normalizeWithOptions(input: input, options: options))

            case "reindexStatusWithOptions":
                guard let dbPath = args["dbPath"] as? String else {
                    return result(badArgs("dbPath:String required"))
                }
                guard let options = normalizeOptions(args, result: result) else { return }
                // The Dart side maps these wire names back to ReindexStatus.
                result(wireName(try reindexStatusWithOptions(dbPath: dbPath, options: options)))

            case "index":
                guard let id = int64(args["id"]) else { return result(badArgs("id:Int required")) }
                guard let text = args["text"] as? String else { return result(badArgs("text:String required")) }
                guard let engine = requireEngine(args, result: result) else { return }
                try engine.index(id: id, text: text)
                result(nil)

            case "remove":
                guard let id = int64(args["id"]) else { return result(badArgs("id:Int required")) }
                guard let engine = requireEngine(args, result: result) else { return }
                try engine.remove(id: id)
                result(nil)

            case "search":
                guard let query = args["query"] as? String else { return result(badArgs("query:String required")) }
                guard let limit = args["limit"] as? Int else { return result(badArgs("limit:Int required")) }
                guard let engine = requireEngine(args, result: result) else { return }
                let hits = try engine.search(query: query, limit: UInt32(limit))
                result(hits.map { ["id": $0.id, "score": $0.score] })

            case "indexRecord":
                guard let recordId = int64(args["recordId"]) else { return result(badArgs("recordId:Int required")) }
                guard let rawFields = args["fields"] as? [[String: Any]] else { return result(badArgs("fields:List required")) }
                guard let engine = requireEngine(args, result: result) else { return }
                let fields = rawFields.compactMap { f -> FieldValue? in
                    guard let slot = (f["slot"] as? NSNumber)?.uint8Value,
                          let text = f["text"] as? String else { return nil }
                    return FieldValue(slot: slot, text: text)
                }
                try engine.indexRecord(recordId: recordId, fields: fields)
                result(nil)

            case "removeRecord":
                guard let recordId = int64(args["recordId"]) else { return result(badArgs("recordId:Int required")) }
                guard let engine = requireEngine(args, result: result) else { return }
                try engine.removeRecord(recordId: recordId)
                result(nil)

            case "searchRecords":
                guard let query = args["query"] as? String else { return result(badArgs("query:String required")) }
                guard let limit = args["limit"] as? Int else { return result(badArgs("limit:Int required")) }
                guard let fieldsPerRecord = args["fieldsPerRecord"] as? Int else { return result(badArgs("fieldsPerRecord:Int required")) }
                guard let engine = requireEngine(args, result: result) else { return }
                let hits = try engine.searchRecords(
                    query: query, limit: UInt32(limit), fieldsPerRecord: UInt32(fieldsPerRecord)
                )
                result(hits.map {
                    ["recordId": $0.recordId, "score": $0.score, "matchedSlots": $0.matchedSlots.map { Int($0) }]
                })

            case "highlight":
                guard let query = args["query"] as? String else { return result(badArgs("query:String required")) }
                guard let id = int64(args["id"]) else { return result(badArgs("id:Int required")) }
                guard let before = args["before"] as? String else { return result(badArgs("before:String required")) }
                guard let after = args["after"] as? String else { return result(badArgs("after:String required")) }
                guard let engine = requireEngine(args, result: result) else { return }
                result(try engine.highlight(query: query, id: id, before: before, after: after))

            case "changeFieldBits":
                guard let newFieldBits = (args["newFieldBits"] as? NSNumber)?.uint8Value else {
                    return result(badArgs("newFieldBits:Int required"))
                }
                guard let engine = requireEngine(args, result: result) else { return }
                let count = try engine.changeFieldBits(newFieldBits: newFieldBits)
                result(Int(count))

            case "dispose":
                guard let handle = args["handle"] as? Int else {
                    return result(badArgs("handle:Int required"))
                }
                engines.removeValue(forKey: handle)
                result(nil)

            default:
                result(FlutterMethodNotImplemented)
            }
        } catch let error as SearchError {
            result(FlutterError(code: "SEARCH_ERROR", message: error.localizedDescription, details: nil))
        } catch {
            result(FlutterError(code: "PLUGIN_ERROR", message: error.localizedDescription, details: nil))
        }
    }

    /// Resolves the engine for `args["handle"]`, or sends a `NO_ENGINE` /
    /// `BAD_ARGS` error and returns nil. Mirrors the Kotlin `NO_ENGINE` code.
    private func requireEngine(_ args: [String: Any], result: @escaping FlutterResult) -> SearchEngine? {
        guard let handle = args["handle"] as? Int else {
            result(badArgs("handle:Int required"))
            return nil
        }
        guard let engine = engines[handle] else {
            result(FlutterError(code: "NO_ENGINE", message: "no engine for handle \(handle)", details: nil))
            return nil
        }
        return engine
    }

    private func badArgs(_ message: String) -> FlutterError {
        FlutterError(code: "BAD_ARGS", message: message, details: nil)
    }

    /// Parses the `options` map (key → Bool) into ``NormalizeOptions``, or sends a
    /// `BAD_ARGS` error and returns nil. Missing flags default to `false`,
    /// matching the Dart `NormalizeOptions` defaults.
    private func normalizeOptions(_ args: [String: Any], result: @escaping FlutterResult) -> NormalizeOptions? {
        guard let map = args["options"] as? [String: Any] else {
            result(badArgs("options:Map required"))
            return nil
        }
        func flag(_ key: String) -> Bool { (map[key] as? NSNumber)?.boolValue ?? false }
        return NormalizeOptions(
            lowercase: flag("lowercase"),
            kanaFold: flag("kanaFold"),
            foldDiacritics: flag("foldDiacritics"),
            foldChoonpu: flag("foldChoonpu"),
            expandIterationMarks: flag("expandIterationMarks"),
            normalizeHyphens: flag("normalizeHyphens"),
            stripDigitGrouping: flag("stripDigitGrouping"),
            collapseWhitespace: flag("collapseWhitespace")
        )
    }

    /// Builds an ``EngineOptionsConfig`` from the call's `options` map and
    /// `strategy` wire name, or sends a `BAD_ARGS` error and returns nil.
    private func engineConfig(_ args: [String: Any], result: @escaping FlutterResult) -> EngineOptionsConfig? {
        guard let options = normalizeOptions(args, result: result) else { return nil }
        guard let strategyName = args["strategy"] as? String else {
            result(badArgs("strategy:String required"))
            return nil
        }
        guard let strategy = searchStrategy(strategyName) else {
            result(badArgs("strategy: unknown value '\(strategyName)'"))
            return nil
        }
        return EngineOptionsConfig(normalize: options, strategy: strategy)
    }

    /// Maps a stable wire name (kept in sync with Dart `SearchStrategy.wireName`
    /// and the Kotlin enum names) to the UniFFI ``SearchStrategy`` case.
    private func searchStrategy(_ wire: String) -> SearchStrategy? {
        switch wire {
        case "TRIGRAM_BM25": return .trigramBm25
        case "SUBSTRING": return .substring
        case "PREFIX": return .prefix
        case "SUFFIX": return .suffix
        case "ALL_TERMS": return .allTerms
        case "FUZZY_TRIGRAM": return .fuzzyTrigram
        case "LEVENSHTEIN": return .levenshtein
        case "DAMERAU_LEVENSHTEIN": return .damerauLevenshtein
        default: return nil
        }
    }

    /// Serializes ``ReindexStatus`` to its stable wire name (matches the Kotlin
    /// enum names and Dart `ReindexStatus.wireName`).
    private func wireName(_ status: ReindexStatus) -> String {
        switch status {
        case .empty: return "EMPTY"
        case .upToDate: return "UP_TO_DATE"
        case .configChanged: return "CONFIG_CHANGED"
        }
    }

    // Flutter's standard message codec boxes numbers as NSNumber; returns nil
    // (rather than crashing) when the value is missing or not numeric.
    private func int64(_ value: Any?) -> Int64? {
        (value as? NSNumber)?.int64Value
    }
}
