import Flutter
import UIKit
import UnifiedQuery

/// iOS side of the Flutter plugin.
///
/// Each open engine lives in ``engines`` keyed by an integer handle that is
/// returned to Dart on "open" and sent back on every subsequent call.
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
                let dbPath = args["dbPath"] as! String
                let engine = try SearchEngine(dbPath: dbPath)
                let handle = nextHandle
                nextHandle += 1
                engines[handle] = engine
                result(handle)

            case "index":
                let engine = try requireEngine(args)
                try engine.index(id: int64(args["id"]!), text: args["text"] as! String)
                result(nil)

            case "remove":
                let engine = try requireEngine(args)
                try engine.remove(id: int64(args["id"]!))
                result(nil)

            case "search":
                let engine = try requireEngine(args)
                let hits = try engine.search(
                    query: args["query"] as! String,
                    limit: UInt32(args["limit"] as! Int)
                )
                result(hits.map { ["id": $0.id, "score": $0.score] })

            case "dispose":
                let handle = args["handle"] as! Int
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

    private func requireEngine(_ args: [String: Any]) throws -> SearchEngine {
        let handle = args["handle"] as! Int
        guard let engine = engines[handle] else {
            throw PluginError.noEngine(handle)
        }
        return engine
    }

    // Flutter's standard message codec boxes numbers as NSNumber.
    private func int64(_ value: Any) -> Int64 {
        (value as! NSNumber).int64Value
    }
}

private enum PluginError: LocalizedError {
    case noEngine(Int)
    var errorDescription: String? {
        switch self {
        case .noEngine(let h): return "no engine for handle \(h)"
        }
    }
}
