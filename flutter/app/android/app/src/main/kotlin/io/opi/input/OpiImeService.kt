package io.opi.input

import android.inputmethodservice.InputMethodService
import android.view.View
import android.view.inputmethod.EditorInfo
import io.flutter.embedding.android.FlutterView
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.embedding.engine.dart.DartExecutor
import io.flutter.FlutterInjector
import io.flutter.plugin.common.MethodChannel

/** OPI IME 宿主：FlutterView 作为输入视图，Dart imeMain entrypoint。 */
class OpiImeService : InputMethodService() {
    private var flutterEngine: FlutterEngine? = null
    private var channel: MethodChannel? = null

    override fun onCreateInputView(): View {
        val engine = FlutterEngine(this)
        val entrypoint = DartExecutor.DartEntrypoint(
            FlutterInjector.instance().flutterLoader().findAppBundlePath(),
            "imeMain"
        )
        engine.dartExecutor.executeDartEntrypoint(entrypoint)

        val view = FlutterView(this)
        view.attachToFlutterEngine(engine)

        channel = MethodChannel(engine.dartExecutor.binaryMessenger, "opi/ime")
        channel?.setMethodCallHandler { call, result ->
            when (call.method) {
                "commitText" -> {
                    currentInputConnection?.commitText(call.arguments as String, 1)
                    result.success(null)
                }
                "deleteBackward" -> {
                    currentInputConnection?.deleteSurroundingText(1, 0)
                    result.success(null)
                }
                "performEnter" -> {
                    currentInputConnection?.performEditorAction(EditorInfo.IME_ACTION_SEND)
                    result.success(null)
                }
                else -> result.notImplemented()
            }
        }

        flutterEngine = engine
        return view
    }

    override fun onDestroy() {
        flutterEngine?.destroy()
        flutterEngine = null
        channel = null
        super.onDestroy()
    }
}
