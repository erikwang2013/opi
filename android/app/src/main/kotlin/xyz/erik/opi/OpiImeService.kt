// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi

import xyz.erik.opi.engine.composingText
import android.inputmethodservice.InputMethodService
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.view.Gravity
import android.util.Log
import android.view.View
import android.view.ViewGroup
import android.view.Window
import android.view.WindowManager
import android.view.inputmethod.EditorInfo
import android.text.InputType // 密码/回车判定（EditorInfoRules.kt 里的纯函数）
import androidx.compose.ui.platform.ComposeView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.LifecycleRegistry
import androidx.savedstate.SavedStateRegistry
import androidx.savedstate.SavedStateRegistryOwner
import xyz.erik.opi.engine.AssociationGate
import xyz.erik.opi.engine.EngineBoot
import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.engine.UserWordStore
import xyz.erik.opi.engine.clearUserWords
import xyz.erik.opi.ime.BracketPairing
import xyz.erik.opi.ime.EngineLoadingView
import xyz.erik.opi.ime.HandlerDebouncer
import xyz.erik.opi.jni.EngineLoader
import xyz.erik.opi.jni.OpiEngine
import xyz.erik.opi.ime.ImeCommitGuard
import xyz.erik.opi.ime.ImeScreen
import xyz.erik.opi.ime.onSelectionChanged
import xyz.erik.opi.ime.ImeState
import xyz.erik.opi.ime.ImeWindowMetrics
import xyz.erik.opi.ime.KeyRouter
import xyz.erik.opi.ime.PendingCommit
import xyz.erik.opi.keyboard.SymbolCatalog
import xyz.erik.opi.keyboard.RareIndex
import xyz.erik.opi.settings.ImePreferences
import java.io.File
// 编辑器信息的两条纯判定（isPasswordEditor / enterDecision）已拆到 EditorInfoRules.kt
//（2026-10-10，500 行门禁；同包，无需 import）。
/** OPI IME 宿主：ComposeView 作为输入视图，面板状态在 ImeState（A3/A4 填 UI）。 */
class OpiImeService : InputMethodService() {
    private var inputViewCache: View? = null
    // 有输入目标：onStartInput(info != null) 置 true，onFinishInput 置 false。
    // 不能按 inputType 判 —— 抖音直播弹幕框用 TYPE_NULL，但仍通过 InputConnection 收字。
    private var editorActive = false
    internal val selfCommitGuard = ImeCommitGuard() // 选区缓存 + 联想收起判据（2026-10-27 搬出）
    /** 提交重试的排程与撤销（字段持有的主线程 Handler，不随 IME 窗口重建而改变）。
     * internal：图片通道（PetStickerCommit.kt 的扩展函数）共用同一套重试/撤销。 */
    internal val pendingCommit = PendingCommit()
    /**
     * 用户词落盘。`by lazy` 是必需的：Service 的字段初始化早于 `attachBaseContext`，
     * 那时 `filesDir` 还不可用（mBase 为 null）—— 首次访问发生在 onCreateInputView
     * 之后（[bootEngine] 的装载线程；第一次选词则仍走主线程），lazy 自带同步。
     */
    internal val userWords: UserWordStore by lazy {
        UserWordStore(
            file = File(filesDir, UserWordStore.FILE_NAME),
            importJson = { OpiEngine.importUserWords(it) },
            exportJson = { OpiEngine.exportUserWords() },
            debouncer = HandlerDebouncer(),
            // 落盘失败此前完全静默：学过的词没存下来，用户和开发者都看不到
            onWriteFailure = { Log.w(TAG, "user words save failed: $it") },
        )
    }
    // internal：同上（ComposingText.kt 的扩展函数要读它）。
    internal val engineController = EngineController(
        onLearned = { userWords.scheduleSave() },
        onBufferSynced = { pushComposingText() },
    )
    /** 装载在 [EngineLoader.io] 上跑，完成经 [mainHandler] 回主线程（[EngineBoot]）。 */
    private val mainHandler = Handler(Looper.getMainLooper())
    private val engineBoot = EngineBoot(EngineLoader.io) { mainHandler.post(it) }

    /** 最近一次推给字段的组合串（提交/清空时要动它，见 [dropComposing] 的说明）。 */
    // internal：组合串助手拆到 ComposingText.kt（拆分裂代价，见那里头注）。
    internal var lastComposing: String = ""

    internal val imeState = ImeState(engineController)
    private val symbolCatalog = SymbolCatalog.androidDefault()
    private lateinit var keyRouter: KeyRouter
    /** 括号自动配对（宿主读写走 InputConnection，见 BracketCommit.kt）。 */
    private val bracketPairing = BracketPairing(AndroidBracketHost(this))

    /** IME 偏好（输入模式/键盘模式/三个引擎开关）。懒初始化：字段初始化早于
     *  `attachBaseContext`，那时 `getSharedPreferences` 拿不到 Context。 */
    internal val prefs: ImePreferences by lazy { ImePreferences.from(this) }

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

    /** 符号页类目（`symbol_categories.tsv`）。懒读；损坏 → 空列表，见 `ImeAssetData`。 */
    private val symbolCategories by lazy { loadSymbolCategories(assets) }

    /** 颜文字分类（`kaomoji.tsv`）。懒读；缺失/损坏 → 空列表，见 `ImeAssetData`。 */
    private val kaomojiCategories by lazy { loadKaomojiCategories(assets) }

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
        internal const val TAG = "OpiImeService"
        /** 悬浮/游戏模式下窗口宽度占屏宽的比例（收窄居中，见 applyKeyboardWindow）。 */
        private const val FLOATING_WIDTH_RATIO = 0.86f
        /** 游戏模式的窗口高度相对常规键盘高度的比例（更矮，少挡画面）。 */
        private const val GAME_HEIGHT_RATIO = 0.72f
    }

    /**
     * IME 窗口高度 = 键区 + 候选栏预留高度（px），计算见 [ImeWindowMetrics]。
     *
     * 候选栏必须计入：它在 ImeScreen 里是**固定挂载**的（见那里的注释），不吃窗口高度就会
     * 从 4 行键里抢走 44dp —— 本机实测候选栏一出现键高 117px → 89px、整块键盘下沉 116px、
     * 每个词跳两次。
     *
     * 2026-10-22：比例/额外高度与横屏、平板、分屏的上限搬进 [ImeWindowMetrics]（纯函数，
     * 有 `ImeWindowMetricsTest` 门禁）。横屏原值占屏高 77%，现封顶 60%；平板封顶 340dp。
     * 竖屏手机的像素与 2026-10-09 裁决的「736→843px」不变（本机口径仍约 831px）。
     */
    private fun keyboardHeight(): Int {
        val (w, h) = windowSizePx()
        return ImeWindowMetrics.keyboardHeightPx(w, h, resources.displayMetrics.density)
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

    override fun onCreate() {
        super.onCreate()
        // 智能联想开关在 App 设置页也能改（同进程、不同控制器）：gate 一变就让本服务
        // 的控制器立即重算候选，否则旧联想候选会留到热重启（2026-10-30 用户报障）。
        AssociationGate.onChange = { engineController.refresh() }
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
        // 引擎/词库装载搬后台（2026-10-28）：此前这段同步占着主线程（首次 36MB 拷贝/校验
        // + Rust install）。装载期只组合骨架键面（EngineLoadingView）；装完 ready 回调回
        // 主线程恢复偏好再切真实键盘 —— 在那之前按键会落到即将被 install 重置的引擎上。
        imeState.persist = {
            prefs.inputMode = imeState.inputMode
            prefs.keyboardModes = imeState.keyboardModes
            prefs.skin = imeState.skin
            prefs.englishPro = imeState.englishPro
            prefs.customPunct = imeState.customPunct
            // 键盘模式里悬浮/游戏改的是**窗口**，改完立刻重排（不等系统下一次 configure）
            window?.window?.let { applyKeyboardWindow(it) }
        }
        engineBoot.start(
            load = { bootEngine() },
            onReady = { ok ->
                engineController.reportDictionaryLoad(ok)
                // 偏好恢复（2026-10-09）：引擎三开关 + 输入模式（方案随它落进引擎）+ 键盘模式。
                // 放这里而不是 onCreate：引擎可能装载失败，偏好是「引擎可用」才有意义的投影。
                prefs.applyEngineSwitches(engineController)
                imeState.restore(prefs.inputMode, prefs.keyboardModes, prefs.skin, prefs.englishPro, prefs.customPunct)
                // restore 可能晚于 onConfigureWindow（窗口先出来）：按恢复后的模式重排一次。
                window?.window?.let { applyKeyboardWindow(it) }
            },
        )
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
            // 提交后重推组合串（2026-10-13「确认后把所有内容替换」的修法，
            // 语义与理由见 KeyRouter.commitAndResync 的头注）。
            syncComposing = { pushComposingText() },
            englishPro = { imeState.englishPro },
            pairing = bracketPairing,
        )
        // IME 提交通道在视图创建时注入（构造期无 this 引用；面板打开提交 pending buffer 用）
        imeState.commit = ::commitWithRetry
        view.setContent {
            if (engineBoot.loading) {
                EngineLoadingView()
            } else {
                ImeScreen(
                    imeState, engineController, keyRouter, symbolCatalog,
                    rareIndex = { rareIndex },
                    symbolCategories = { symbolCategories },
                    kaomojiCategories = { kaomojiCategories },
                    // 「自有」宠物形态 = 插入 PNG 图片（文本通道之外，见 PetStickerCommit.kt）。
                    onPickPet = { commitPetSticker(it) },
                    // 工具栏最右「收起键盘」（2026-10-10 用户裁决）：收回系统键盘。
                    onCloseKeyboard = { requestHideSelf(0) },
                    // 键盘内设置面板的「清除用户词库」：同一个 store、同一套两半清理。
                    onClearUserWords = { clearUserWords(userWords) },
                )
            }
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
            // 2026-10-26：选区来自 onUpdateSelection 缓存。旧实现每次按 ⌫ 都同步
            // getSelectedText(0)（binder 往返目标应用，慢编辑器数 ms）；只有缓存
            // 说选区非空时才需要动选区。
            if (selfCommitGuard.selectionNonEmpty) {
                ic.commitText("", 1)
            } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
                ic.deleteSurroundingTextInCodePoints(1, 0)
            } else {
                ic.deleteSurroundingText(1, 0)
            }
        }
    }

    override fun onUpdateSelection(
        oldSelStart: Int,
        oldSelEnd: Int,
        newSelStart: Int,
        newSelEnd: Int,
        candidatesStart: Int,
        candidatesEnd: Int,
    ) {
        super.onUpdateSelection(oldSelStart, oldSelEnd, newSelStart, newSelEnd, candidatesStart, candidatesEnd)
        selfCommitGuard.onSelectionChanged(engineController, newSelStart, newSelEnd)
    }

    /** 回车：读目标应用声明的 action；action 位为 0 时回退提交换行。 */
    private fun performEnter() {
        // P6：硬编码 SEND 会把"搜索"键发成发送；读目标应用声明的 action。
        // 2026-10-10 用户报障「换行无效」——判据改为 [enterDecision]（多行编辑器惯例
        // IME_FLAG_NO_ENTER_ACTION 优先于 action 位：真实世界不少应用两样都设，旧逻辑
        // 会把它们的回车发成 action 而不是换行；主流输入法/AOSP 均先看这个 flag）。
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
        editorActive = info != null
        if (!restarting) {
            Log.i(TAG, "onStartInput: editor changed")
            // 排队中的重试提交必须在这里撤掉：它与组合串同属「上一个编辑器的东西」
            cancelPendingCommit()
            // 换编辑器后，上一处插入的配对不能再被当成「我们的」于新编辑器里删改
            bracketPairing.reset()
            imeState.onEditorChanged()
            // 取消组合串（不提交，避免半截拼音泄入新编辑器）
            engineController.clear()
            // 密码类字段自动临时英文（2026-10-10 用户报障「弹框里没法输入」的第二来源；
            // 主流输入法同场景自动切英文）。普通编辑器则还原上一处密码场的临时切换。
            val pw = info != null && isPasswordEditor(info.inputType)
            Log.i(TAG, "onStartInput: inputType=${info?.inputType} password=$pw")
            if (pw) {
                imeState.enterTransientEnglish()
            } else {
                imeState.leaveTransientEnglish()
            }
        }
    }

    override fun onFinishInput() {
        super.onFinishInput(); editorActive = false; Log.i(TAG, "onFinishInput")
    }

    override fun onFinishInputView(finishingInput: Boolean) {
        super.onFinishInputView(finishingInput)
        Log.i(TAG, "onFinishInputView")
        // 普通 hide（返回键/同一编辑器重开）也会走到本方法，此时 finishingInput=false。
        // 这里只管「真正结束输入」那档（清缓冲/清 ⇧/回 qwerty）；普通 hide 的
        // **视图重置**归 onWindowHidden（2026-10-17 裁决：隐藏整块键盘就重置到默认界面
        // —— 早先「避免收起键盘误清 SYMBOL 面板状态」的取舍已被该裁决推翻）。
        if (finishingInput) {
            // 同 onStartInput：结束输入时排队中的重试同样不能再落到下一个编辑器
            cancelPendingCommit()
            bracketPairing.reset()
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
     * 没法输入，主流输入法就可以」——它们走系统默认判定，从不进这个状态）。
     *
     * 2026-10-21 再次修订：不能按 inputType 判 —— 抖音直播弹幕框用 TYPE_NULL，
     * 旧判据让它永远返回 false。现在只认生命周期（onStartInput/onFinishInput），
     * 弹框/无编辑器窗口拿不到 onStartInput，仍会收起键盘，2026-10-10 的中间态不会回来。
     */
    override fun onEvaluateInputViewShown(): Boolean {
        val verdict = editorActive && currentInputConnection != null
        Log.i(
            TAG,
            "onEvaluateInputViewShown: info=${currentInputEditorInfo?.inputType} " +
                "active=$editorActive conn=${currentInputConnection != null} " +
                "shown=$isInputViewShown -> $verdict",
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
        // 2026-10-17 裁决：隐藏即重置到默认界面（覆盖返回键/收起键/切应用；不碰引擎缓冲）。
        imeState.resetToDefaultView()
        lifecycleOwner.registry.currentState = Lifecycle.State.STARTED
    }

    override fun onDestroy() {
        // 开关变化回调不能留住已销毁的服务/控制器
        AssociationGate.onChange = null
        // 销毁前撤掉未执行的提交重试，避免向新输入框误提交
        cancelPendingCommit()
        bracketPairing.reset()
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
