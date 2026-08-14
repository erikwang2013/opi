package io.opi.input

import android.inputmethodservice.InputMethodService
import android.os.Build
import android.util.Log
import android.view.View
import android.view.ViewGroup
import android.view.Window
import android.view.inputmethod.EditorInfo
import androidx.compose.ui.platform.ComposeView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.LifecycleRegistry
import androidx.savedstate.SavedStateRegistryOwner
import io.opi.input.engine.EngineController
import io.opi.input.ime.ImeScreen
import io.opi.input.ime.ImeState
import io.opi.input.ime.KeyRouter
import io.opi.input.keyboard.SymbolCatalog
import kotlin.math.min

/** OPI IME 宿主：ComposeView 作为输入视图，面板状态在 ImeState（A3/A4 填 UI）。 */
class OpiImeService : InputMethodService() {
    private var inputViewCache: View? = null
    private var retryRunnable: Runnable? = null
    private val engineController = EngineController()
    private val imeState = ImeState(engineController)
    private val symbolCatalog = SymbolCatalog()
    private lateinit var keyRouter: KeyRouter

    // Compose 需要 ViewTreeLifecycleOwner/SavedStateRegistryOwner（IME Service 无 Activity 生命周期）。
    // 最小实现：lifecycleRegistry 手动推进（CREATED→RESUMED→STARTED→DESTROYED）。
    private val lifecycleOwner = ImeLifecycleOwner()
    // savedstate 1.2.1 的 SavedStateRegistry() 为 Kotlin internal（Java 桥实例化）；
    // SavedStateRegistryOwner 继承 LifecycleOwner，lifecycle 委托给同一个 lifecycleOwner。
    private val savedStateRegistryOwner = object : SavedStateRegistryOwner {
        override val savedStateRegistry = SavedStateRegistryFactory.create()
        override val lifecycle: Lifecycle get() = lifecycleOwner.lifecycle
    }

    companion object {
        private const val TAG = "OpiImeService"
        // 曲面屏底部圆角 r≈147px（dumpsys RoundedCorners），底行键落弧区外触发不到；
        // 参考系统输入法留底部安全区（Gboard 同款做法：按键上移、背景延伸到底）。
        private const val BOTTOM_SAFE_PX = 168
    }

    private fun keyboardHeight(): Int {
        val dm = resources.displayMetrics
        // 横屏 0.42×宽 会超屏高（2400×0.42+168 > 1080），底行面板切换键被裁出屏外；
        // 基数取 min(宽,高)，再钳制在屏高内（留底部安全区）。
        val side = min(dm.widthPixels, dm.heightPixels)
        val computed = (side * 0.42).toInt() + BOTTOM_SAFE_PX
        return computed.coerceAtMost(dm.heightPixels - BOTTOM_SAFE_PX)
    }

    override fun onCreateInputView(): View {
        // P1 防重入：窗口每次重建（模式切换/重新显示）都会再调本方法，
        // 每次新建视图会丢状态并泄漏旧视图。缓存复用。
        val cached = inputViewCache
        if (cached != null) {
            Log.i(TAG, "onCreateInputView: reuse cached view")
            return cached
        }
        Log.i(TAG, "onCreateInputView: start")
        val view = ComposeView(this)
        // 生命周期接线：置 CREATED（onWindowShown→RESUMED、onWindowHidden→STARTED、
        // onDestroy→DESTROYED），Compose 侧 remember/LaunchedEffect 依赖它。
        // ViewTree* 是 Kotlin object（api-jar transform 剥 metadata 后 Kotlin 不可解析），走 Java 桥。
        ViewTreeBridge.setLifecycleOwner(view, lifecycleOwner)
        ViewTreeBridge.setSavedStateRegistryOwner(view, savedStateRegistryOwner)
        lifecycleOwner.registry.currentState = Lifecycle.State.CREATED
        keyRouter = KeyRouter(
            controller = engineController,
            commit = ::commitWithRetry,
            deleteBackward = ::deleteBackward,
            performEnter = ::performEnter,
        )
        // IME 提交通道在视图创建时注入（构造期无 this 引用；面板打开提交 pending buffer 用）
        imeState.commit = ::commitWithRetry
        view.setContent { ImeScreen(imeState, engineController, keyRouter, symbolCatalog) }
        Log.i(TAG, "onCreateInputView: screenW=${resources.displayMetrics.widthPixels} keyboardHeight=${keyboardHeight()}")
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
        // 先撤掉上一次未执行的 retryRunnable，避免两次重试叠加（onDestroy 也靠它回收）。
        retryRunnable?.let { window.window?.decorView?.removeCallbacks(it) }
        val r = Runnable {
            val ic2 = currentInputConnection
            if (ic2 != null) ic2.commitText(text, 1)
            else Log.w(TAG, "IC still null, commit dropped: \"$text\"")
        }
        retryRunnable = r
        if (window.window == null) {
            Log.w(TAG, "window.window null, commit retry dropped: \"$text\"")
        } else {
            window.window?.decorView?.postDelayed(r, 50)
        }
    }

    /** 删除：有选区先删选区；无选区按码点删（emoji 等代理对不拆半）。 */
    private fun deleteBackward() {
        Log.i(TAG, "deleteBackward")
        val ic = currentInputConnection
        if (ic != null) {
            val sel = ic.getSelectedText(0)
            if (!sel.isNullOrEmpty()) {
                ic.commitText("", 1)
            } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
                ic.deleteSurroundingTextInCodePoints(1, 0)
            } else {
                ic.deleteSurroundingText(1, 0)
            }
        }
    }

    /** 回车：读目标应用声明的 action；action 位为 0 时回退提交换行。 */
    private fun performEnter() {
        // P6：硬编码 SEND 会把"搜索"键发成发送；读目标应用声明的 action。
        // action 位为 0（应用未声明 action / 仅设 NO_ENTER_ACTION / 编辑器信息缺失）时
        // performEditorAction(0) 是 no-op → 回车键死亡，回退提交换行（不硬编码 SEND）。
        val action = currentInputEditorInfo?.imeOptions?.and(EditorInfo.IME_MASK_ACTION)
            ?: 0
        Log.i(TAG, "performEnter action=$action")
        val ic = currentInputConnection
        if (ic != null) {
            if (action != 0) ic.performEditorAction(action)
            else ic.commitText("\n", 1)
        }
    }

    /** P1：输入目标切换/输入视图结束时重置面板状态（组合串、候选、面板视图）。 */
    override fun onStartInput(info: EditorInfo?, restarting: Boolean) {
        super.onStartInput(info, restarting)
        if (!restarting) {
            Log.i(TAG, "onStartInput: editor changed")
            imeState.onEditorChanged()
            // 取消组合串（不提交，避免半截拼音泄入新编辑器）
            engineController.clear()
        }
    }

    override fun onFinishInputView(finishingInput: Boolean) {
        super.onFinishInputView(finishingInput)
        Log.i(TAG, "onFinishInputView")
        // 普通 hide（返回键/同一编辑器重开）也会走到本方法，此时 finishingInput=false；
        // 只在真正结束输入（销毁/停用）时重置，避免收起键盘误清 SYMBOL 面板状态。
        if (finishingInput) {
            imeState.onEditorChanged()
            engineController.clear()
        }
    }

    override fun onConfigureWindow(win: Window, isFullscreen: Boolean, isCandidatesOnly: Boolean) {
        super.onConfigureWindow(win, isFullscreen, isCandidatesOnly)
        // 默认实现非全屏时设 WRAP_CONTENT，ComposeView 在 AT_MOST 下量出全屏
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
        // Compose 是普通 View 渲染：无 FlutterSurfaceView 重建问题，仅推进生命周期。
        lifecycleOwner.registry.currentState = Lifecycle.State.RESUMED
    }

    override fun onWindowHidden() {
        super.onWindowHidden()
        Log.i(TAG, "onWindowHidden")
        lifecycleOwner.registry.currentState = Lifecycle.State.STARTED
    }

    override fun onDestroy() {
        // 销毁前撤掉未执行的提交重试，避免向新输入框误提交
        retryRunnable?.let { window.window?.decorView?.removeCallbacks(it) }
        retryRunnable = null
        lifecycleOwner.registry.currentState = Lifecycle.State.DESTROYED
        inputViewCache = null
        super.onDestroy()
    }
}

/** IME Service 最小 LifecycleOwner：registry 由 Service 手动推进状态。 */
private class ImeLifecycleOwner : LifecycleOwner {
    val registry: LifecycleRegistry = LifecycleRegistry(this)
    override val lifecycle: Lifecycle get() = registry
}
