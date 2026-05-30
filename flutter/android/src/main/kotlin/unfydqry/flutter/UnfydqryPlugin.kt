package unfydqry.flutter

// ─── Maintenance guide (for iOS developers) ──────────────────────────────────
// This file is the Android counterpart of ios/Classes/UnfydqryPlugin.swift.
// You should rarely need to change it. The only reason to edit it is when the
// native SearchEngine API changes:
//
//   If `uniffi.unfydqry.SearchEngine` gains, removes, or renames a method,
//   this file will FAIL TO COMPILE and the error will point to the exact line.
//   Mirror the same change you made to UnfydqryPlugin.swift — the method
//   names and parameters are the same; only the syntax differs.
//
// The Kotlin syntax cheat-sheet for this file:
//   fun name(param: Type): ReturnType        →  func name(param: Type) -> ReturnType
//   call.argument<String>("key") ?: bad(...) →  args["key"] as? String ?? bad(...)
//   result.success(value)                    →  result(value)
//
// Threading: the method channel delivers calls on the platform main thread, so
// `engines` and `nextHandle` need no synchronization.
// ─────────────────────────────────────────────────────────────────────────────

import io.flutter.embedding.engine.plugins.FlutterPlugin
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import io.flutter.plugin.common.MethodChannel.MethodCallHandler
import io.flutter.plugin.common.MethodChannel.Result
import uniffi.unfydqry.SearchEngine
import uniffi.unfydqry.SearchException

/**
 * Android side of the Flutter plugin.
 *
 * Each open engine is kept in [engines] under an integer handle that is
 * returned to Dart on 'open' and passed back on every subsequent call.
 */
class UnfydqryPlugin : FlutterPlugin, MethodCallHandler {

    private lateinit var channel: MethodChannel
    private val engines = mutableMapOf<Int, SearchEngine>()
    private var nextHandle = 0

    override fun onAttachedToEngine(binding: FlutterPlugin.FlutterPluginBinding) {
        channel = MethodChannel(binding.binaryMessenger, "unfydqry/search")
        channel.setMethodCallHandler(this)
    }

    override fun onDetachedFromEngine(binding: FlutterPlugin.FlutterPluginBinding) {
        channel.setMethodCallHandler(null)
        engines.values.forEach { it.close() }
        engines.clear()
    }

    override fun onMethodCall(call: MethodCall, result: Result) {
        try {
            when (call.method) {
                "open" -> {
                    val dbPath = call.argument<String>("dbPath")
                        ?: return result.badArgs("dbPath:String required")
                    val handle = nextHandle++
                    engines[handle] = SearchEngine(dbPath)
                    result.success(handle)
                }

                "index" -> {
                    val id = call.longArg("id") ?: return result.badArgs("id:Int required")
                    val text = call.argument<String>("text")
                        ?: return result.badArgs("text:String required")
                    val engine = engine(call, result) ?: return
                    engine.index(id = id, text = text)
                    result.success(null)
                }

                "remove" -> {
                    val id = call.longArg("id") ?: return result.badArgs("id:Int required")
                    val engine = engine(call, result) ?: return
                    engine.remove(id = id)
                    result.success(null)
                }

                "search" -> {
                    val query = call.argument<String>("query")
                        ?: return result.badArgs("query:String required")
                    val limit = call.argument<Int>("limit")
                        ?: return result.badArgs("limit:Int required")
                    val engine = engine(call, result) ?: return
                    val hits = engine.search(query = query, limit = limit.toUInt())
                    result.success(hits.map { mapOf("id" to it.id, "score" to it.score) })
                }

                "dispose" -> {
                    val handle = call.argument<Int>("handle")
                        ?: return result.badArgs("handle:Int required")
                    engines.remove(handle)?.close()
                    result.success(null)
                }

                else -> result.notImplemented()
            }
        } catch (e: SearchException) {
            result.error("SEARCH_ERROR", e.message, null)
        } catch (e: Exception) {
            result.error("PLUGIN_ERROR", e.message, null)
        }
    }

    /** Resolves the engine for the call's `handle`, or sends a `NO_ENGINE` error and returns null. */
    private fun engine(call: MethodCall, result: Result): SearchEngine? {
        val handle = call.argument<Int>("handle")
            ?: run { result.badArgs("handle:Int required"); return null }
        return engines[handle]
            ?: run { result.error("NO_ENGINE", "no engine for handle $handle", null); return null }
    }

    private fun Result.badArgs(message: String) = error("BAD_ARGS", message, null)

    // Flutter's method channel can deliver Dart int as Int or Long depending on value;
    // returns null (rather than throwing) when the value is missing or not numeric.
    private fun MethodCall.longArg(key: String): Long? = (argument<Any>(key) as? Number)?.toLong()
}
