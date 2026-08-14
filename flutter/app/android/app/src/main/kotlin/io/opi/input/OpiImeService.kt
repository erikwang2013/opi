package io.opi.input

import android.inputmethodservice.InputMethodService
import android.view.View
import android.view.ViewGroup
import android.view.Window
import android.view.inputmethod.EditorInfo
import android.util.Log
import io.flutter.embedding.android.FlutterView
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.embedding.engine.dart.DartExecutor
import io.flutter.FlutterInjector
import io.flutter.plugin.common.MethodChannel

/** OPI IME 宿主：FlutterView 作为输入视图，Dart imeMain entrypoint。 */
class OpiImeService : InputMethodService() {
    private var flutterEngine: FlutterEngine? = null
    private var inputViewCache: View? = null
    private var channel: MethodChannel? = null

    companion object {
        private const val TAG = "OpiImeService"
        // 曲面屏底部圆角 r≈147px（dumpsys RoundedCorners），底行键落弧区外触发不到；
        // 参考系统输入法留底部安全区（Gboard 同款做法：按键上移、背景延伸到底）。
        private const val BOTTOM_SAFE_PX = 168
    }

    private fun keyboardHeight(): Int =
        (resources.displayMetrics.widthPixels * 0.42).toInt() + BOTTOM_SAFE_PX

    override fun onCreateInputView(): View {
        // P1 防重入：窗口每次重建（模式切换/重新显示）都会再调本方法，
        // 每次新建 FlutterEngine 会空窗 0.5-2s 吞触摸并泄漏旧 engine。缓存复用。
        val cached = inputViewCache
        if (cached != null && flutterEngine != null) {
            Log.i(TAG, "onCreateInputView: reuse cached view/engine")
            return cached
        }
        Log.i(TAG, "onCreateInputView: start")
        // FlutterActivity 会在 onAttach 中自动初始化 FlutterLoader；IME 服务没有该流程。
        // 缺省时 libflutter.so 不加载、attachToNative 失败，FlutterView 全黑无帧。
        // （MainActivity 同进程先跑过时内部有 initialized 守卫，重复调用是幂等 no-op。）
        val flutterLoader = FlutterInjector.instance().flutterLoader()
        flutterLoader.startInitialization(this)
        flutterLoader.ensureInitializationComplete(this, null)
        Log.i(TAG, "onCreateInputView: flutter loader ready")

        val engine = FlutterEngine(this)
        // 缺省（2 参构造）时引擎只在根库 main.dart 里按名找 entrypoint，
        // imeMain 在独立库里会 "Could not resolve main entrypoint function"（键盘全黑）。
        val entrypoint = DartExecutor.DartEntrypoint(
            FlutterInjector.instance().flutterLoader().findAppBundlePath(),
            "package:app/ime/ime_main.dart",
            "imeMain"
        )
        engine.dartExecutor.executeDartEntrypoint(entrypoint)
        Log.i(TAG, "onCreateInputView: entrypoint executed")

        val view = FlutterView(this)
        view.attachToFlutterEngine(engine)

        val keyboardHeight = keyboardHeight()
        Log.i(TAG, "onCreateInputView: screenW=${resources.displayMetrics.widthPixels} keyboardHeight=$keyboardHeight")
        view.addOnLayoutChangeListener { _, l, t, r, b, _, _, _, _ ->
            Log.i(TAG, "inputView layout: ${r - l}x${b - t}")
        }
        window.window?.decorView?.addOnLayoutChangeListener { _, l, t, r, b, _, _, _, _ ->
            Log.i(TAG, "imeWindow layout: ${r - l}x${b - t}")
        }

        channel = MethodChannel(engine.dartExecutor.binaryMessenger, "opi/ime")
        channel?.setMethodCallHandler { call, result ->
            when (call.method) {
                "commitText" -> {
                    Log.i(TAG, "channel commitText len=${(call.arguments as String).length}")
                    commitWithRetry(call.arguments as String)
                    result.success(null)
                }
                "deleteBackward" -> {
                    Log.i(TAG, "channel deleteBackward")
                    currentInputConnection?.deleteSurroundingText(1, 0)
                    result.success(null)
                }
                "performEnter" -> {
                    // P6：硬编码 SEND 会把"搜索"键发成发送；读目标应用声明的 action
                    val action = currentInputEditorInfo?.imeOptions?.and(EditorInfo.IME_MASK_ACTION)
                        ?: EditorInfo.IME_ACTION_SEND
                    Log.i(TAG, "channel performEnter action=$action")
                    currentInputConnection?.performEditorAction(action)
                    result.success(null)
                }
                else -> result.notImplemented()
            }
        }

        flutterEngine = engine
        inputViewCache = view
        return view
    }

    /** P2：IC 为 null 时提交被静默丢弃（白屏/启动期触摸无响应），延迟 50ms 重试一次。 */
    private fun commitWithRetry(text: String) {
        val ic = currentInputConnection
        if (ic != null) {
            ic.commitText(text, 1)
            return
        }
        Log.w(TAG, "IC null, retry commit in 50ms: \"$text\"")
        window.window?.decorView?.postDelayed({
            val ic2 = currentInputConnection
            if (ic2 != null) ic2.commitText(text, 1)
            else Log.w(TAG, "IC still null, commit dropped: \"$text\"")
        }, 50)
    }

    override fun onConfigureWindow(win: Window, isFullscreen: Boolean, isCandidatesOnly: Boolean) {
        super.onConfigureWindow(win, isFullscreen, isCandidatesOnly)
        // 默认实现非全屏时设 WRAP_CONTENT，FlutterView 在 AT_MOST 下量出全屏 3032
        // 导致窗口盖住被输入应用；窗口首次显示及每次模式变化都会走到这里，强制键盘高度。
        win.setLayout(ViewGroup.LayoutParams.MATCH_PARENT, keyboardHeight())
    }

    override fun onComputeInsets(outInsets: Insets) {
        super.onComputeInsets(outInsets)
        Log.i(TAG, "onComputeInsets visibleTop=${outInsets.visibleTopInsets} contentTop=${outInsets.contentTopInsets}")
    }

    override fun onWindowShown() {
        super.onWindowShown()
        Log.i(TAG, "onWindowShown")
    }

    override fun onWindowHidden() {
        super.onWindowHidden()
        Log.i(TAG, "onWindowHidden")
    }

    override fun onDestroy() {
        flutterEngine?.destroy()
        flutterEngine = null
        inputViewCache = null
        channel = null
        super.onDestroy()
    }
}
