// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi

import android.inputmethodservice.InputMethodService
import android.os.Build
import android.view.Gravity
import android.util.Log
import android.view.View
import android.view.ViewGroup
import android.view.Window
import android.view.WindowManager
import android.view.inputmethod.EditorInfo
import android.text.InputType
import androidx.compose.ui.platform.ComposeView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.LifecycleRegistry
import androidx.savedstate.SavedStateRegistry
import androidx.savedstate.SavedStateRegistryOwner
import xyz.erik.opi.candidate.CANDIDATE_BAR_HEIGHT_DP
import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.engine.UserWordStore
import xyz.erik.opi.ime.HandlerDebouncer
import xyz.erik.opi.jni.EngineLoader
import xyz.erik.opi.jni.OpiEngine
import xyz.erik.opi.ime.ImeScreen
import xyz.erik.opi.ime.ImeState
import xyz.erik.opi.ime.KeyRouter
import xyz.erik.opi.ime.PendingCommit
import xyz.erik.opi.keyboard.SymbolCatalog
import xyz.erik.opi.keyboard.RareIndex
import xyz.erik.opi.settings.ImePreferences
import java.io.File
import kotlin.math.min

/**
 * 密码类编辑器判定（2026-10-10）：**文本类**里的 PASSWORD / VISIBLE_PASSWORD /
 * WEB_PASSWORD 三种变体。纯函数，单测钉边界。
 * ⚠️ 数字类密码（TYPE_CLASS_NUMBER|TYPE_NUMBER_VARIATION_PASSWORD）**不在列**——
 * 数字面板默认直出数字，没有「被拼音吃掉」的问题，别顺手扩进来。
 */
internal fun isPasswordEditor(inputType: Int): Boolean {
    if (inputType and InputType.TYPE_MASK_CLASS != InputType.TYPE_CLASS_TEXT) return false
    return when (inputType and InputType.TYPE_MASK_VARIATION) {
        InputType.TYPE_TEXT_VARIATION_PASSWORD,
        InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD,
        InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD,
        -> true
        else -> false
    }
}

/** 回车键的决策（纯函数，单测钉）。 */
internal sealed interface EnterDecision {
    /** 执行应用声明的编辑器 action（搜索/发送/下一步…）。 */
    data class PerformAction(val action: Int) : EnterDecision

    /** 提交换行（多行编辑器、或应用未声明 action）。 */
    object Newline : EnterDecision
}

/**
 * 回车决策（2026-10-10 用户报障「换行无效」的订正）：
 * **IME_FLAG_NO_ENTER_ACTION 优先于 action 位** —— 多行编辑器惯例上两样都设
 * （NO_ENTER_ACTION + 一个残留的 action），旧逻辑只看 action 会把它们的回车发成
 * 动作而不是换行；搜狗/AOSP 的判法都是先看这个 flag。纯函数，边界由单测钉。
 */
internal fun enterDecision(imeOptions: Int): EnterDecision {
    val action = imeOptions and EditorInfo.IME_MASK_ACTION
    val noEnterAction = (imeOptions and EditorInfo.IME_FLAG_NO_ENTER_ACTION) != 0
    // 两个「没有动作」的编码都要走换行：UNSPECIFIED=0（未声明）与 NONE=1（明确说没有）。
    val hasAction = action != EditorInfo.IME_ACTION_UNSPECIFIED &&
        action != EditorInfo.IME_ACTION_NONE
    return if (!noEnterAction && hasAction) {
        EnterDecision.PerformAction(action)
    } else {
        EnterDecision.Newline
    }
}

/** OPI IME 宿主：ComposeView 作为输入视图，面板状态在 ImeState（A3/A4 填 UI）。 */
class OpiImeService : InputMethodService() {
    private var inputViewCache: View? = null

    /** 提交重试的排程与撤销（字段持有的主线程 Handler，不随 IME 窗口重建而改变）。 */
    private val pendingCommit = PendingCommit()

    /**
     * 用户词落盘。`by lazy` 是必需的：Service 的字段初始化早于 `attachBaseContext`，
     * 那时 `filesDir` 还不可用（mBase 为 null）—— 首次访问发生在 onCreateInputView
     * 之后（显式 load() 或第一次选词），两条路径都在主线程。
     */
    private val userWords: UserWordStore by lazy {
        UserWordStore(
            file = File(filesDir, UserWordStore.FILE_NAME),
            importJson = { OpiEngine.importUserWords(it) },
            exportJson = { OpiEngine.exportUserWords() },
            debouncer = HandlerDebouncer(),
            // 落盘失败此前完全静默：学过的词没存下来，用户和开发者都看不到
            onWriteFailure = { Log.w(TAG, "user words save failed: $it") },
        )
    }
    private val engineController = EngineController(
        onLearned = { userWords.scheduleSave() },
        onBufferSynced = ::pushComposingText,
    )

    /** 最近一次推给字段的组合串（提交/清空时要动它，见 [dropComposing] 的说明）。 */
    private var lastComposing: String = ""

    /**
     * 组合串进字段（2026-10-10 用户裁决「需要」）：打字期间输入框里能看到拼音
     * （搜狗式）——此前组合串只画在自家候选栏，字段里空空；用户以为没打进去。
     * 文本用**显示读数**（`reading`：9 键 = `ni'hao` 这类分词拼音；其余 = 原字母）。
     * 英文模式字母直传不经过缓冲 ⇒ 无组合串，正确。
     *
     * ⚠️ 缓冲空时**先 `setComposingText("")` 再 `finishComposingText`** —— 只 finish
     * 会把组合区里**最后那段文字落定留在字段里**（退格到空会剩半截拼音）。
     */
    private fun pushComposingText() {
        val ic = currentInputConnection ?: return
        val text = engineController.reading.ifEmpty { engineController.buffer }
        if (text.isEmpty()) {
            ic.setComposingText("", 1)
            ic.finishComposingText()
            lastComposing = ""
        } else {
            ic.setComposingText(text, 1)
            lastComposing = text
        }
    }

    /**
     * 提交前把上次推的组合串从字段里抹掉（2026-10-10 实测「ni'hao你好」的修法）。
     *
     * 次序成因：选词路径是「引擎 select（缓冲清空）→ refresh（组合区判空 ⇒ 落定成
     * 普通文本）→ 调用方 commitText」——commitText 到得比组合区落定晚，替换不了它，
     * 只能追加，字段里留下拼音痕迹。先把 lastComposing 那串删掉，**两种状态都得到
     * 「替换」的净效果**（组合区仍活跃时删的就是它；已落定时删的是刚落定的那串）。
     * 删除按码点（组合串是拼音/字母，ASCII 为主，按码点与退格同一条纪律）。
     */
    private fun dropComposing(ic: android.view.inputmethod.InputConnection) {
        if (lastComposing.isEmpty()) return
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            ic.deleteSurroundingTextInCodePoints(lastComposing.length, 0)
        } else {
            ic.deleteSurroundingText(lastComposing.length, 0)
        }
        lastComposing = ""
    }
    private val imeState = ImeState(engineController)
    private val symbolCatalog = SymbolCatalog()
    private lateinit var keyRouter: KeyRouter

    /** IME 偏好（输入模式/键盘模式/三个引擎开关）。懒初始化：字段初始化早于
     *  `attachBaseContext`，那时 `getSharedPreferences` 拿不到 Context。 */
    private val prefs: ImePreferences by lazy { ImePreferences.from(this) }

    /**
     * 生僻字索引（1.1MB TSV）。**懒加载到第一次真正打开生僻字面板** —— 别挪进
     * `onCreateInputView`：那是每次键盘出现都走的热路径。资产损坏 → 空索引（面板
     * 显示「无数据」而不是崩 IME），与词库装载同一条降级姿态。
     */
    private val rareIndex: Map<Int, Map<Int, List<String>>> by lazy {
        try {
            assets.open("rare_index.tsv").bufferedReader().use { RareIndex.parse(it.readText()) }
        } catch (e: Exception) {
            Log.w(TAG, "rare_index.tsv 读取失败，生僻字面板将为空: $e")
            emptyMap()
        }
    }

    /** 符号页类目（`assets/symbol_categories.tsv`，生成物入库）。同 rareIndex：懒读、
     * 损坏 → 空列表（页面显示「没有类目数据」而不是崩 IME）。 */
    private val symbolCategories: List<xyz.erik.opi.keyboard.SymbolCategories.Category> by lazy {
        try {
            assets.open("symbol_categories.tsv").bufferedReader()
                .use { xyz.erik.opi.keyboard.SymbolCategories.parse(it.readText()) }
        } catch (e: Exception) {
            Log.w(TAG, "symbol_categories.tsv 读取失败，符号页将为空: $e")
            emptyList()
        }
    }

    // Compose 需要 ViewTreeLifecycleOwner（IME Service 无 Activity 生命周期）。
    // 最小实现：lifecycleRegistry 手动推进（CREATED→RESUMED→STARTED→DESTROYED）。
    // SavedStateRegistryOwner 必须挂：Compose 1.7+ 的 AndroidComposeView.onAttachedToWindow
    // 要求 propagateViewTreeSavedStateRegistryOwner，否则抛 IllegalStateException；
    // registry 需已 restore（consumeRestoredStateForKey 检查），而 1.2.1 的 performRestore
    // 为 mangled internal 无法源码调用 → SavedStateRegistryFactory 反射置 isRestored=true
    // （IME 无持久状态，等效空 restore）。
    private val lifecycleOwner = ImeLifecycleOwner()
    private val savedStateRegistryOwner = object : SavedStateRegistryOwner {
        override val savedStateRegistry: SavedStateRegistry =
            SavedStateRegistryFactory.createRestored()
        override val lifecycle: Lifecycle get() = lifecycleOwner.lifecycle
    }

    companion object {
        private const val TAG = "OpiImeService"
        /** 键区高度占可用窗口短边的比例（对齐 flutter ime_main.dart 的 0.42）。 */
        private const val KEY_AREA_RATIO = 0.48
        /**
         * 键区在比例基数之外额外加的高度（dp）。
         *
         * 原为裸像素 168，注释称它是「底部安全区（按键上移、背景延伸到底）」—— 实测不成立：
         * 窗口高度是常量，多出来的 168px 被 ImeScreen 里 4 行 weight(1f) 均分掉了，底行键
         * 照样画到窗口最底（本机实测键底 y=2270，窗口底 y=2400）。真正把键区抬到手势条之上
         * 的是**系统导航栏 inset**（本机实测 129px，decorFitsSystemWindows 自动下的 padding），
         * 与这个常量无关，所以这里不需要再留"安全区"。
         * 它现在的身份就是「键区加高」，按 dp 取实测等效值。
         * 2026-10-09 用户裁决「整体布局 UI 太小，没有搜狗输入法移动端显示大」：
         * 64 → 76dp（连同比率 0.42 → 0.48），本机 1080×2400 实测键盘总高
         * **736px → 843px**（占屏 30.7% → 35.1%，往搜狗/GBoard 的 40% 量级靠）；
         * 键行 ≈91px → ≈106px、字母字号 18 → 21sp（见 NineKeyKeyboard/KeyButton）。
         * 裸像素在 density 1.0 上是 199dp、3.5 上是 57dp，差 3.5 倍。
         */
        private const val KEY_AREA_EXTRA_DP = 76

        /** 悬浮/游戏模式下窗口宽度占屏宽的比例（收窄居中，见 applyKeyboardWindow）。 */
        private const val FLOATING_WIDTH_RATIO = 0.86f
        /** 游戏模式的窗口高度相对常规键盘高度的比例（更矮，少挡画面）。 */
        private const val GAME_HEIGHT_RATIO = 0.72f
    }

    /**
     * IME 窗口高度 = 键区 + 候选栏预留高度（px）。
     *
     * 候选栏必须计入：它在 ImeScreen 里是**固定挂载**的（见那里的注释），不吃窗口高度就会
     * 从 4 行键里抢走 44dp —— 本机实测候选栏一出现键高 117px → 89px、整块键盘下沉 116px、
     * 每个词跳两次。
     *
     * 代价（刻意的取舍，别为了数字好看改回去）：窗口多出候选栏那 44dp ——
     * 竖屏实测 621→736px、占屏 25.9%→30.7%（Gboard 约 40%，同量级）；
     * **横屏键区占屏高由 57% 升到 68%，未在真横屏机上验**。
     * 换成"从键区里扣 44dp"会让键高锁死在 34dp，低于 44dp 触控目标，所以不换。
     */
    private fun keyboardHeight(): Int {
        val density = resources.displayMetrics.density
        val (w, h) = windowSizePx()
        // 横屏 0.42×宽 会超屏高（2400×0.42 > 1080），底行面板切换键被裁出屏外；
        // 基数取短边，再钳制在可用窗口高度内。
        val side = min(w, h)
        val keyArea = (side * KEY_AREA_RATIO).toInt() + (KEY_AREA_EXTRA_DP * density).toInt()
        val bar = (CANDIDATE_BAR_HEIGHT_DP * density).toInt()
        return (keyArea + bar).coerceAtMost(h)
    }

    /**
     * 可用窗口尺寸（px）。API 30+ 用 `currentWindowMetrics.bounds`：`resources.displayMetrics`
     * 报的是**整屏**，分屏/折叠屏/自由窗口下会高估，键盘按整屏算就会盖住应用。
     */
    private fun windowSizePx(): Pair<Int, Int> {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            val b = getSystemService(WindowManager::class.java).currentWindowMetrics.bounds
            return b.width() to b.height()
        }
        val dm = resources.displayMetrics
        return dm.widthPixels to dm.heightPixels
    }

    /** Compose 1.7+ 的 getWindowRecomposer 在 IME 窗口根（decorView/parentPanel 链）上查找
     *  ViewTreeLifecycleOwner——只设在 ComposeView 自身会崩
     *  （"ViewTreeLifecycleOwner not found from parentPanel"）。addView 进窗口前挂到
     *  decorView（窗口根），子视图查找即可命中；ComposeView 自身设置保留（低版本路径）。 */
    override fun setInputView(view: View) {
        // InputMethodService.getWindow() 返回 Dialog（IME 窗口用 Dialog 实现）。
        // LifecycleOwner 与（已 restore 的）SavedStateRegistryOwner 都要挂到窗口根，
        // Compose 1.7+ 的 AndroidComposeView 在父链上查找两者，缺任一即抛异常。
        val root: View? = getWindow()?.window?.decorView
        if (root != null) {
            ViewTreeBridge.setLifecycleOwner(root, lifecycleOwner)
            ViewTreeBridge.setSavedStateRegistryOwner(root, savedStateRegistryOwner)
        }
        // 旋转必崩的根因，在 AOSP 那侧：resetStateForNewConfiguration 会换一棵全新 root、
        // 把 mInputFrame 指向一个新的空 FrameLayout、并置 mInputView = null，但**从不**
        // 摘掉挂在被丢弃的旧 root 上的旧 view。随后 updateInputViewShown 见 mInputView
        // 为 null 又调 onCreateInputView()，P1 缓存把同一实例交回来，而 super.setInputView
        // 里的 mInputFrame.removeAllViews() 清的是那个新空 frame（空操作）→ addView 抛
        // "The specified child already has a parent"。
        // 所以进 frame 前先把自己从旧 parent 上摘干净。修在这里而不是 onCreateInputView：
        // setInputView 是"视图进入 frame"的唯一收口，AOSP 自调用与将来任何调用方都覆盖到。
        (view.parent as? ViewGroup)?.removeView(view)
        super.setInputView(view)
    }

    override fun onCreateInputView(): View {
        // P1 防重入：窗口每次重建（模式切换/重新显示）都会再调本方法，每次新建视图会丢状态
        // （面板/候选/生命周期接线全部重来）。复用本身是安全的 —— 安全性由 setInputView
        // 负责：它会把视图先摘离旧 parent。这条路径真正的坑是"复用却没摘"，不是"新建"。
        val cached = inputViewCache
        if (cached != null) {
            Log.i(TAG, "onCreateInputView: reuse cached view")
            return cached
        }
        Log.i(TAG, "onCreateInputView: start")
        // luna 词库编排（幂等：size 校验重拷；失败回退内置词库；与设置页共享 Rust 单例）
        // 返回值是词库装载结果（false = 回退到内置 35 词库），接住它喂给宠物：降级时
        // 候选栏待命位的小欧折断天线（DEGRADED），而不是照旧笑嘻嘻。
        engineController.reportDictionaryLoad(EngineLoader.load(this))
        // 用户词导入必须在词库装载之后、建视图之前：文件缺失/损坏静默降级（不崩 IME），
        // 且要赶在第一次刷新候选之前生效
        userWords.load()
        // 偏好恢复（2026-10-09）：引擎三开关 + 输入模式（方案随它落进引擎）+ 键盘模式。
        // 放这里而不是 onCreate：引擎可能装载失败，偏好是「引擎可用」才有意义的投影。
        prefs.applyEngineSwitches(engineController)
        imeState.persist = {
            prefs.inputMode = imeState.inputMode
            prefs.keyboardModes = imeState.keyboardModes
            prefs.skin = imeState.skin
            // 键盘模式里悬浮/游戏改的是**窗口**，改完立刻重排（不等系统下一次 configure）
            window?.window?.let { applyKeyboardWindow(it) }
        }
        imeState.restore(prefs.inputMode, prefs.keyboardModes, prefs.skin)
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
        view.setContent {
            ImeScreen(
                imeState, engineController, keyRouter, symbolCatalog,
                rareIndex = { rareIndex },
                symbolCategories = { symbolCategories },
                // 工具栏最右「收起键盘」（2026-10-10 用户裁决）：收回系统键盘。
                onCloseKeyboard = { requestHideSelf(0) },
            )
        }
        Log.i(TAG, "onCreateInputView: window=${windowSizePx()} keyboardHeight=${keyboardHeight()}")
        inputViewCache = view
        return view
    }

    /** P2：IC 为 null 时提交被静默丢弃（白屏/启动期触摸无响应），延迟 50ms 重试一次。 */
    private fun commitWithRetry(text: String) {
        val ic = currentInputConnection
        if (ic != null) {
            dropComposing(ic)
            ic.commitText(text, 1)
            return
        }
        Log.w(TAG, "IC null, retry commit in ${PendingCommit.RETRY_MS}ms: \"$text\"")
        // 排程与撤销都在 pendingCommit 内部，共用同一个 Handler —— 不再经 decorView
        pendingCommit.schedule {
            val ic2 = currentInputConnection
            if (ic2 != null) {
                dropComposing(ic2)
                ic2.commitText(text, 1)
            } else {
                Log.w(TAG, "IC still null, commit dropped: \"$text\"")
            }
        }
    }

    /**
     * 撤掉排队中的提交重试。
     *
     * 输入目标切换（`onStartInput`）、输入视图结束（`onFinishInputView`）与销毁时都必须调：
     * 排队中的提交在延迟后才执行，期间编辑器可能已经换掉 —— 不撤就会把上一次的候选
     * 文字写进**新的**编辑框。
     */
    private fun cancelPendingCommit() = pendingCommit.cancel()

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
        // 2026-10-10 用户报障「换行无效」——判据改为 [enterDecision]（多行编辑器惯例
        // IME_FLAG_NO_ENTER_ACTION 优先于 action 位：真实世界不少应用两样都设，旧逻辑
        // 会把它们的回车发成 action 而不是换行；搜狗/AOSP 均先看这个 flag）。
        val opts = currentInputEditorInfo?.imeOptions ?: 0
        val decision = enterDecision(opts)
        Log.i(TAG, "performEnter opts=$opts -> $decision")
        val ic = currentInputConnection ?: return
        when (decision) {
            is EnterDecision.PerformAction -> ic.performEditorAction(decision.action)
            EnterDecision.Newline -> {
                dropComposing(ic)
                ic.commitText("\n", 1)
            }
        }
    }

    /** P1：输入目标切换/输入视图结束时重置面板状态（组合串、候选、面板视图）。 */
    override fun onStartInput(info: EditorInfo?, restarting: Boolean) {
        super.onStartInput(info, restarting)
        if (!restarting) {
            Log.i(TAG, "onStartInput: editor changed")
            // 排队中的重试提交必须在这里撤掉：它与组合串同属「上一个编辑器的东西」
            cancelPendingCommit()
            imeState.onEditorChanged()
            // 取消组合串（不提交，避免半截拼音泄入新编辑器）
            engineController.clear()
            // 密码类字段自动临时英文（2026-10-10 用户报障「弹框里没法输入」的第二来源；
            // 搜狗同场景自动切英文）。普通编辑器则还原上一处密码场的临时切换。
            val pw = info != null && isPasswordEditor(info.inputType)
            Log.i(TAG, "onStartInput: inputType=${info?.inputType} password=$pw")
            if (pw) {
                imeState.enterTransientEnglish()
            } else {
                imeState.leaveTransientEnglish()
            }
        }
    }

    override fun onFinishInputView(finishingInput: Boolean) {
        super.onFinishInputView(finishingInput)
        Log.i(TAG, "onFinishInputView")
        // 普通 hide（返回键/同一编辑器重开）也会走到本方法，此时 finishingInput=false；
        // 只在真正结束输入（销毁/停用）时重置，避免收起键盘误清 SYMBOL 面板状态。
        if (finishingInput) {
            // 同 onStartInput：结束输入时排队中的重试同样不能再落到下一个编辑器
            cancelPendingCommit()
            imeState.onEditorChanged()
            engineController.clear()
        }
    }

    override fun onConfigureWindow(win: Window, isFullscreen: Boolean, isCandidatesOnly: Boolean) {
        super.onConfigureWindow(win, isFullscreen, isCandidatesOnly)
        // 默认实现非全屏时设 WRAP_CONTENT，ComposeView 在 AT_MOST 下量出全屏
        // 导致窗口盖住被输入应用；窗口首次显示及每次模式变化都会走到这里，强制键盘高度。
        // 悬浮/游戏模式的窗口收窄/变矮也在这里统一施加（见 applyKeyboardWindow）。
        applyKeyboardWindow(win)
    }

    /**
     * 键盘窗口布局（2026-10-09，「键盘模式」里的**窗口层**两种）：
     *   · 悬浮模式：窗口收窄到屏宽 [FLOATING_WIDTH_RATIO]、水平居中、贴底；
     *   · 游戏模式：窗口更矮（[GAME_HEIGHT_RATIO] × 常规高度）+ 同样收窄居中，
     *     尽量少挡画面。
     * 默认（都关）：全宽 + 常规高度，与 2026-10-09 之前逐像素一致。
     *
     * ⚠️ **本机无真机**：窗口尺寸/重力只能真机看效果（分屏、手势条 inset 都可能影响）。
     * 实现按 Android IME 窗口惯例（setAttributes 改 gravity/width + setLayout 设尺寸），
     * 如实标在 docs/install/android.md；悬浮**不做拖动**（那要处理触摸重设 layout，
     * 没有真机的反馈链不写这个）。
     */
    private fun applyKeyboardWindow(win: Window) {
        val modes = imeState.keyboardModes
        val height = if (modes.game) {
            (keyboardHeight() * GAME_HEIGHT_RATIO).toInt().coerceAtLeast(1)
        } else {
            keyboardHeight()
        }
        val lp = win.attributes
        if (modes.floating || modes.game) {
            val (w, _) = windowSizePx()
            lp.width = (w * FLOATING_WIDTH_RATIO).toInt()
            win.attributes = lp
            win.setGravity(Gravity.BOTTOM or Gravity.CENTER_HORIZONTAL)
        } else {
            lp.width = ViewGroup.LayoutParams.MATCH_PARENT
            win.attributes = lp
            win.setGravity(Gravity.BOTTOM)
        }
        win.setLayout(lp.width, height)
    }

    /**
     * 输入视图显示判定（2026-10-10 修订）：**有「接受文本」的编辑器才返回 true**。
     *
     * 历史：原实现无条件 `= true` —— 理由是模拟器上报硬件键盘（qwerty）时默认实现
     * 返回 false、onCreateInputView 永不调用（键盘空壳）。但无条件 true 还有另一面：
     * 弹框 / 无编辑器的窗口抢焦时，系统想让键盘收起，我们仍把它亮着，留下
     * 「**键盘可见、触摸全无反应**」的中间态（2026-10-10 用户报障「其他 app 有弹框就
     * 没法输入，搜狗就可以」——搜狗走系统默认判定，从不进这个状态）。
     *
     * 改法：当前编辑器接受文本（inputType 类 ≠ TYPE_NULL）且输入连接在 ⇒ true
     * （模拟器/硬键盘场景由这半保住）；其余一律 false 交回系统 —— 弹框抢焦、
     * 无编辑器时键盘正常收起，不再留中间态。
     */
    override fun onEvaluateInputViewShown(): Boolean {
        val info = currentInputEditorInfo
        val acceptsText = info != null &&
            (info.inputType and InputType.TYPE_MASK_CLASS) != InputType.TYPE_NULL
        val verdict = acceptsText && currentInputConnection != null
        Log.i(
            TAG,
            "onEvaluateInputViewShown: info=${info?.inputType} acceptsText=$acceptsText " +
                "conn=${currentInputConnection != null} shown=$isInputViewShown -> $verdict",
        )
        return verdict
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
        cancelPendingCommit()
        // 取消 pending 搜索防抖（销毁后回调不应再触发）
        imeState.onEditorChanged()
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
