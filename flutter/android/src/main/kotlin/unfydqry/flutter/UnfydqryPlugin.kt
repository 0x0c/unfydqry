package unfydqry.flutter

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
                    val dbPath = call.argument<String>("dbPath")!!
                    val handle = nextHandle++
                    engines[handle] = SearchEngine(dbPath)
                    result.success(handle)
                }

                "index" -> {
                    engine(call).index(
                        id = call.longArg("id"),
                        text = call.argument<String>("text")!!,
                    )
                    result.success(null)
                }

                "remove" -> {
                    engine(call).remove(id = call.longArg("id"))
                    result.success(null)
                }

                "search" -> {
                    val hits = engine(call).search(
                        query = call.argument<String>("query")!!,
                        limit = call.argument<Int>("limit")!!.toUInt(),
                    )
                    result.success(hits.map { mapOf("id" to it.id, "score" to it.score) })
                }

                "dispose" -> {
                    val handle = call.argument<Int>("handle")!!
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

    private fun engine(call: MethodCall): SearchEngine =
        engines[call.argument<Int>("handle")!!]
            ?: error("no engine for handle ${call.argument<Int>("handle")}")

    // Flutter's method channel can deliver Dart int as Int or Long depending on value.
    private fun MethodCall.longArg(key: String): Long =
        (argument<Any>(key) as Number).toLong()
}
