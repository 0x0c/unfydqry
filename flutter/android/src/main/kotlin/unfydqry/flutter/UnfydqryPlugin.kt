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
import uniffi.unfydqry.EngineOptionsConfig
import uniffi.unfydqry.FieldValue
import uniffi.unfydqry.NormalizeOptions
import uniffi.unfydqry.SearchEngine
import uniffi.unfydqry.SearchException
import uniffi.unfydqry.SearchStrategy
import uniffi.unfydqry.normalizeWithOptions
import uniffi.unfydqry.reindexStatusWithOptions

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

                "openWithOptions" -> {
                    val dbPath = call.argument<String>("dbPath")
                        ?: return result.badArgs("dbPath:String required")
                    val config = call.engineConfig(result) ?: return
                    val handle = nextHandle++
                    engines[handle] = SearchEngine.withOptions(dbPath, config)
                    result.success(handle)
                }

                "openWithOptionsRebuilding" -> {
                    val dbPath = call.argument<String>("dbPath")
                        ?: return result.badArgs("dbPath:String required")
                    val config = call.engineConfig(result) ?: return
                    val handle = nextHandle++
                    engines[handle] = SearchEngine.withOptionsRebuilding(dbPath, config)
                    result.success(handle)
                }

                "normalizeWithOptions" -> {
                    val input = call.argument<String>("input")
                        ?: return result.badArgs("input:String required")
                    val options = call.normalizeOptions(result) ?: return
                    result.success(normalizeWithOptions(input, options))
                }

                "reindexStatusWithOptions" -> {
                    val dbPath = call.argument<String>("dbPath")
                        ?: return result.badArgs("dbPath:String required")
                    val options = call.normalizeOptions(result) ?: return
                    // The Dart side maps these enum names back to ReindexStatus.
                    result.success(reindexStatusWithOptions(dbPath, options).name)
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

                "indexRecord" -> {
                    val recordId = call.longArg("recordId")
                        ?: return result.badArgs("recordId:Int required")
                    val rawFields = call.argument<List<Map<String, Any>>>("fields")
                        ?: return result.badArgs("fields:List required")
                    val engine = engine(call, result) ?: return
                    val fields = rawFields.mapNotNull { f ->
                        val slot = (f["slot"] as? Number)?.toInt() ?: return@mapNotNull null
                        val text = f["text"] as? String ?: return@mapNotNull null
                        FieldValue(slot = slot.toUByte(), text = text)
                    }
                    engine.indexRecord(recordId = recordId, fields = fields)
                    result.success(null)
                }

                "removeRecord" -> {
                    val recordId = call.longArg("recordId")
                        ?: return result.badArgs("recordId:Int required")
                    val engine = engine(call, result) ?: return
                    engine.removeRecord(recordId = recordId)
                    result.success(null)
                }

                "searchRecords" -> {
                    val query = call.argument<String>("query")
                        ?: return result.badArgs("query:String required")
                    val limit = call.argument<Int>("limit")
                        ?: return result.badArgs("limit:Int required")
                    val fieldsPerRecord = call.argument<Int>("fieldsPerRecord")
                        ?: return result.badArgs("fieldsPerRecord:Int required")
                    val engine = engine(call, result) ?: return
                    val hits = engine.searchRecords(
                        query = query,
                        limit = limit.toUInt(),
                        fieldsPerRecord = fieldsPerRecord.toUInt(),
                    )
                    result.success(
                        hits.map {
                            mapOf(
                                "recordId" to it.recordId,
                                "score" to it.score,
                                "matchedSlots" to it.matchedSlots.map { s -> s.toInt() },
                            )
                        },
                    )
                }

                "highlight" -> {
                    val query = call.argument<String>("query")
                        ?: return result.badArgs("query:String required")
                    val id = call.longArg("id") ?: return result.badArgs("id:Int required")
                    val before = call.argument<String>("before")
                        ?: return result.badArgs("before:String required")
                    val after = call.argument<String>("after")
                        ?: return result.badArgs("after:String required")
                    val engine = engine(call, result) ?: return
                    result.success(engine.highlight(query = query, id = id, before = before, after = after))
                }

                "changeFieldBits" -> {
                    val newFieldBits = call.argument<Int>("newFieldBits")
                        ?: return result.badArgs("newFieldBits:Int required")
                    val engine = engine(call, result) ?: return
                    val count = engine.changeFieldBits(newFieldBits = newFieldBits.toUByte())
                    result.success(count.toLong())
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

    /**
     * Parses the `options` map (key → Boolean) into [NormalizeOptions], or sends a
     * `BAD_ARGS` error and returns null. Missing flags default to `false`,
     * matching the Dart [NormalizeOptions] defaults.
     */
    private fun MethodCall.normalizeOptions(result: Result): NormalizeOptions? {
        val map = argument<Map<String, Any>>("options")
            ?: run { result.badArgs("options:Map required"); return null }
        fun flag(key: String) = map[key] as? Boolean ?: false
        return NormalizeOptions(
            lowercase = flag("lowercase"),
            kanaFold = flag("kanaFold"),
            foldDiacritics = flag("foldDiacritics"),
            foldChoonpu = flag("foldChoonpu"),
            expandIterationMarks = flag("expandIterationMarks"),
            normalizeHyphens = flag("normalizeHyphens"),
            stripDigitGrouping = flag("stripDigitGrouping"),
            collapseWhitespace = flag("collapseWhitespace"),
        )
    }

    /**
     * Builds an [EngineOptionsConfig] from the call's `options` map and `strategy`
     * string (the enum name), or sends a `BAD_ARGS` error and returns null.
     */
    private fun MethodCall.engineConfig(result: Result): EngineOptionsConfig? {
        val options = normalizeOptions(result) ?: return null
        val strategyName = argument<String>("strategy")
            ?: return run { result.badArgs("strategy:String required"); null }
        val strategy = runCatching { SearchStrategy.valueOf(strategyName) }.getOrNull()
            ?: return run { result.badArgs("strategy: unknown value '$strategyName'"); null }
        return EngineOptionsConfig(normalize = options, strategy = strategy)
    }
}
