// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 不进任务栏/菜单栏：把状态栏/虚拟键盘/符号页这类**漂浮小窗**从任务栏、窗口列表里摘掉
//! （用户裁决 2026-10-10 下午：「桌面端状态栏是不在菜单栏显示图标的」；2026-10-13 符号/表情页
//! 也归入**输入工具窗**一类跟上 —— 先例是虚拟键盘。设置窗**不贴**：那是用户要找的窗）。
//! 焦点不受影响：SKIP_* 只决定任务栏归属，符号页按需 `focusable`（搜索打字）那条路照旧。
//!
//! 【为什么不用 `java.awt.Window.setType(UTILITY)` —— 先试过，实证在 JDK 侧就是死的，别再回头】
//! JDK 18 的 XAWT 里 `XWindowPeer.preInit` 抄了 `target.getType()`（XWindowPeer.java:158），
//! 但字段声明自带初始化 `private Window.Type windowType = Window.Type.NORMAL;`（:121），
//! 编译出来的赋值在**构造器里跑在 super 链之后**，把 preInit 刚抄进去的值原样覆掉 ——
//! 于是 AWT 每次 map 都写 `_NET_WM_WINDOW_TYPE_NORMAL`。本机取证（见提交报告）：
//! `javap` 真发行的 XWindowPeer.class 只有 3 处 `putfield windowType`（两个构造器 = NORMAL
//! 字面量、preInit = 拷贝），探针同刻读 `frame.getType()=UTILITY` / `peer.getWindowType()=NORMAL`，
//! xprop 读回 NORMAL。**结论：这条路不是 deepin 的 WM 不吃，是 JDK 里没人能生效。**
//!
//! 所以走 X11 属性直写（JNA 已是既有依赖，不引新库）：
//! · `_NET_WM_WINDOW_TYPE = [_NET_WM_WINDOW_TYPE_UTILITY]` —— 本机实测该 WM 接受且不覆写
//!   （手工 xprop 写入后 6s 读回仍是 UTILITY）；
//! · `_NET_WM_STATE` **读-改-写、只追加** `SKIP_TASKBAR`/`SKIP_PAGER` —— 保留 AWT 写的
//!   `ABOVE`/`STAYS_ON_TOP`（整条重写会砸掉置顶，[appendMissing] 因此保序保留既有项）；
//! · 找窗：X 里按 `_NET_WM_PID` == 本进程 且 `_NET_WM_NAME` == 窗口标题 BFS（WM 会 reparent，
//!   客户端窗在根之下 ≥2 层；用标题区分是因为同进程不止一条这种窗）。
//!
//! AWT 在**每次** map（含隐藏后再显示）都重写一遍 ⇒ 本模块也每次显示后重贴：窗口的
//! shown/open 事件先到就先贴（**连发** 0/20/60/150ms，见 [SkipTaskbar.reapplySoon]），
//! 1s 轮询兜底（与状态栏读状态的节奏同拍，最迟 1s 收敛）。
//! 所有 X 调用都只走本模块这一个专用线程（Xlib 没开 XInitThreads —— 2026-10-13 用 `nm -D`
//! 扫遍 `$JAVA_HOME/lib/*.so`（含 libawt_xawt）对 `XInitThreads` 零引用，同一条连接别多线程碰）。
//!
//! 尽力而为：连不上 X（如 Wayland 原生）/ 找不到窗口时**静默跳过**，绝不影响窗口本身的功能。
//! 【取证状态】属性路线本机实测有效（写后 xprop 读回 UTILITY + SKIP_*，且隐藏-再显示后
//! 1s 内被重贴）；Windows 侧整个模块是 no-op（本机无 Windows，没跑过，也不该跑）。

package xyz.erik.opi.candidate

import com.sun.jna.Library
import com.sun.jna.Memory
import com.sun.jna.Native
import com.sun.jna.Pointer
import com.sun.jna.ptr.IntByReference
import com.sun.jna.ptr.LongByReference
import com.sun.jna.ptr.PointerByReference
import java.awt.Window
import java.awt.event.ComponentAdapter
import java.awt.event.ComponentEvent
import java.awt.event.WindowAdapter
import java.awt.event.WindowEvent
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit
import java.util.concurrent.Executors

/** EWMH 原子名（线上契约字面量；拼错 = 静默无效，所以有单测钉住）。 */
internal const val ATOM_WM_WINDOW_TYPE = "_NET_WM_WINDOW_TYPE"
internal const val ATOM_WM_WINDOW_TYPE_UTILITY = "_NET_WM_WINDOW_TYPE_UTILITY"
internal const val ATOM_WM_STATE = "_NET_WM_STATE"
internal const val ATOM_WM_STATE_SKIP_TASKBAR = "_NET_WM_STATE_SKIP_TASKBAR"
internal const val ATOM_WM_STATE_SKIP_PAGER = "_NET_WM_STATE_SKIP_PAGER"
internal const val ATOM_WM_PID = "_NET_WM_PID"
internal const val ATOM_WM_NAME = "_NET_WM_NAME"

/** Xlib 里要用的函数（直映一个是一个，不多映 —— 与 KeyInject.kt 同一纪律）。 */
internal interface XTaskbarApi : Library {
    fun XOpenDisplay(name: String?): Pointer?
    fun XDefaultRootWindow(dpy: Pointer): Long
    fun XInternAtom(dpy: Pointer, name: String, onlyIfExists: Int): Long
    fun XFlush(dpy: Pointer): Int
    fun XFree(data: Pointer?): Int

    /** format = 32 时 data 按 **long 数组**传（Xlib 文档口径，LP64 上每项 8 字节）。 */
    fun XChangeProperty(
        dpy: Pointer, window: Long, property: Long, type: Long, format: Int,
        mode: Int, data: LongArray, nelements: Int,
    ): Int

    fun XGetWindowProperty(
        dpy: Pointer, window: Long, property: Long, offset: Long, length: Long, delete: Int,
        reqType: Long, actualType: PointerByReference, actualFormat: IntByReference,
        nitems: LongByReference, bytesAfter: LongByReference, prop: PointerByReference,
    ): Int

    fun XQueryTree(
        dpy: Pointer, window: Long, root: LongByReference, parent: LongByReference,
        children: PointerByReference, nchildren: IntByReference,
    ): Int
}

/**
 * X11 输入区域（像素）。**只给状态栏的「错落溢出」用**：AWT `Window.setShape`
 * 只改 X11 的 bounding shape（2026-10-30 本机 XShape 实测：input rects 仍是整窗），
 * 透明边距照样吃点击 ⇒ 条外装饰必须再用 `XShapeCombineRectangles(ShapeInput)`
 * 收成条本体那一块，装饰才真正透传。
 */
internal data class InputRect(val x: Int, val y: Int, val width: Int, val height: Int)

/** libXext 的 shape 入口（只映一个函数，与 [XTaskbarApi] 同一纪律）。 */
internal interface XShapeApi : Library {
    fun XShapeCombineRectangles(
        dpy: Pointer, dest: Long, destKind: Int, xOff: Int, yOff: Int,
        rects: Pointer, nRects: Int, ordering: Int, operation: Int,
    )
}

private const val XA_ATOM = 4L // X11/Xatom.h 的预定义原子号，不是随手挑的数
private const val PROP_MODE_REPLACE = 0
private const val ANY_PROPERTY_TYPE = 0L
private const val TICK_MS = 1000L // 与状态栏读状态的轮询同拍
// X11 shape.h：ShapeBounding=0 / ShapeClip=1 / ShapeInput=2；ShapeSet=0；Unsorted=0。
private const val SHAPE_INPUT = 2
private const val SHAPE_SET = 0
private const val UNSORTED = 0

/** 显示事件后的连发补贴时刻（ms）：把「AWT 稍后重写属性」的兜底窗口从 1s 压到 150ms。 */
private val BURST_MS = longArrayOf(0, 20, 60, 150)

/** 纯函数：把 [wanted] 里还不在 [existing] 的原子**追加**在后面（保序、既有项一个不动）。 */
internal fun appendMissing(existing: List<Long>, wanted: List<Long>): List<Long> =
    existing + wanted.filter { it != 0L && it !in existing }

/** 纯函数：这条窗是不是我们的目标。pid 取不到（属性缺失）时只认标题。 */
internal fun windowMatches(name: String?, pid: Long?, myPid: Long, title: String): Boolean =
    name == title && (pid == null || pid == myPid)

/**
 * 给一条漂浮窗贴「不进任务栏」属性。幂等：属性已是目标值就一个字节也不写。
 * 在窗口**创建时**调一次即可（内部自己跟随每次显示重贴）。
 */
internal object SkipTaskbar {

    /** 标题 → 认下的窗 ID（0 = 还没找到）。找到一次就缓存，省得每秒全树扫。 */
    private val tracked = ConcurrentHashMap<String, Long>()

    /** 单线程执行器：所有 X 调用都在这条线程上；轮询兜底每 [TICK_MS] 补一次。 */
    private val worker by lazy {
        Executors.newSingleThreadScheduledExecutor { r ->
            Thread(r, "opi-skip-taskbar").apply { isDaemon = true }
        }.also { ex ->
            ex.scheduleWithFixedDelay(
                { tracked.keys.toList().forEach(::ensure) }, TICK_MS, TICK_MS, TimeUnit.MILLISECONDS,
            )
        }
    }

    /** X 连接缓存在 lazy 里（只试一次；连不上也不重试 —— 每次开一遍连接没意义）。 */
    private val conn: Pair<XTaskbarApi, Pointer>? by lazy {
        try {
            val api = Native.load("X11", XTaskbarApi::class.java)
            val dpy = api.XOpenDisplay(null)
            if (dpy == null) null else api to dpy
        } catch (t: Throwable) {
            null
        }
    }

    /** libXext（输入形状）；没有它也不影响窗口，装饰透传退化为「会被点击到」。 */
    private val shapeApi: XShapeApi? by lazy {
        try {
            Native.load("Xext", XShapeApi::class.java)
        } catch (t: Throwable) {
            null
        }
    }

    /** 标题 → 条本体的输入矩形（像素）。只在状态栏调用。 */
    private val inputShapes = ConcurrentHashMap<String, List<InputRect>>()

    /**
     * 把点击区域收成 [rects]（状态栏 = 条本体）。视觉形状仍是 `Window.shape`
     * （条+椭圆岛），但 X11 的输入形状单独设置后，条外装饰不再吃点击。
     * 与 SkipTaskbar 共用单线程 Xlib 连接（同一条连接不能多线程碰）。
     */
    fun applyInputShape(window: Window, rects: List<InputRect>) {
        if (isWindows()) return
        val title = (window as? java.awt.Frame)?.title ?: return
        inputShapes[title] = rects
        reapplySoon(title)
    }

    fun apply(window: Window) {
        if (isWindows()) return
        // 标题只在 Frame/Dialog 上（java.awt.Window 自己没有）—— 三条窗都是 Frame（ComposeWindow）。
        val title = (window as? java.awt.Frame)?.title ?: return
        // 同一标题的新窗 = 新 XID（虚拟键盘关了再开就是这条），旧缓存作废。
        tracked[title] = 0L
        reapplySoon(title)
        // 每次显示后 AWT 都会把窗口类型重写回 NORMAL ⇒ 显示事件先到就先贴，连发+轮询兜底。
        window.addWindowListener(object : WindowAdapter() {
            override fun windowOpened(e: WindowEvent) { reapplySoon(title) }
        })
        window.addComponentListener(object : ComponentAdapter() {
            override fun componentShown(e: ComponentEvent) { reapplySoon(title) }
        })
    }

    /**
     * 显示后的**连发补贴**：0/20/60/150ms 在 worker 这条线程上各补一拍（show 事件本身也走它）。
     *
     * 为什么不「在显示事件里**同步**补」（原本最想做的形态）：那要求 Xlib 线程安全，而本机
     * JDK 18.0.2.1 **没调过 XInitThreads** —— `nm -D` 扫遍 `$JAVA_HOME/lib` 下的每个
     * `.so`（含 libawt_xawt）对 `XInitThreads` **零引用**，libX11 的全局锁也就没开；EDT 与 worker 同时
     * （⚠ 这行本来写的是 `$JAVA_HOME/lib` 再接一个通配写法：块注释里出现「斜杠+星号」会
     * 当场嵌套下去把后面整段吞成注释 —— Kotlin 的块注释**可嵌套**，2026-10-13 踩过一次。）
     * 碰同一条 X 连接是 UB ⇒ 同步路的前提不成立，不硬上。连发保持「单线程串行 + 同一条连接」。
     *
     * 连发的目标是「AWT 的重写发生在我们这一拍**之后**」这个竞态。2026-10-13 5ms 粒度实测
     * （hide→re-show，元件在 /tmp/opi-skip-{before,after}-5ms.log）：本机 AWT 的重写发生在
     * 窗口**还 unmapped** 时、事件驱动的那一拍晚 7.6ms 到、映射前 17ms 属性已正确 ——
     * 所以连发在本机观测不到差异，是防「别处 JDK/WM 顺序相反」的保险（1s 轮询仍是最后兜底）。
     */
    private fun reapplySoon(title: String) {
        BURST_MS.forEach { d -> worker.schedule({ ensure(title) }, d, TimeUnit.MILLISECONDS) }
    }

    private fun ensure(title: String) {
        if (isWindows()) return
        val (api, dpy) = conn ?: return
        val cached = tracked[title] ?: return // 已注销的标题
        if (cached != 0L) {
            if (patch(api, dpy, cached)) {
                applyInputShapeFor(api, dpy, cached, title)
                return
            }
            // 这条窗没了（虚拟键盘关掉就是这条）：退回「还没找到」，别每秒去拍死 XID。
            // ponytail: 没找到期间每秒一次全树扫；一条窗关着不开就这么跑，树是几百条的量级。
            tracked[title] = 0L
        }
        val xid = findWindow(api, dpy, title) ?: return // 还没 map / 已经没了：下轮再来
        tracked[title] = xid
        patch(api, dpy, xid)
        applyInputShapeFor(api, dpy, xid, title)
    }

    /**
     * 设置 X11 输入形状。rects 为空/没有 libXext/不是状态栏 ⇒ 不碰（窗口保持整窗
     * 可点）。ShapeSet + Unsorted：每次整体替换，不用维护增量。
     */
    private fun applyInputShapeFor(api: XTaskbarApi, dpy: Pointer, xid: Long, title: String) {
        val rects = inputShapes[title] ?: return
        if (rects.isEmpty()) return
        val shape = shapeApi ?: return
        // XRectangle = 4 × short = 8 字节（x/y/width/height，均为 16 位；窗口尺寸远小于 32767）。
        val mem = Memory(rects.size * 8L)
        for (i in rects.indices) {
            val r = rects[i]
            val off = i * 8L
            mem.setShort(off, r.x.coerceIn(0, Short.MAX_VALUE.toInt()).toShort())
            mem.setShort(off + 2, r.y.coerceIn(0, Short.MAX_VALUE.toInt()).toShort())
            mem.setShort(off + 4, r.width.coerceIn(0, Short.MAX_VALUE.toInt()).toShort())
            mem.setShort(off + 6, r.height.coerceIn(0, Short.MAX_VALUE.toInt()).toShort())
        }
        shape.XShapeCombineRectangles(
            dpy, xid, SHAPE_INPUT, 0, 0, mem, rects.size, UNSORTED, SHAPE_SET,
        )
        api.XFlush(dpy)
    }

    /** 贴属性；返回 false = 这条窗读不回属性（不再存在），调用方应重新找。 */
    private fun patch(api: XTaskbarApi, dpy: Pointer, xid: Long): Boolean {
        val typeProp = api.XInternAtom(dpy, ATOM_WM_WINDOW_TYPE, 0)
        val utility = api.XInternAtom(dpy, ATOM_WM_WINDOW_TYPE_UTILITY, 0)
        val cur = readAtoms(api, dpy, xid, typeProp) ?: return false
        if (cur != listOf(utility)) {
            api.XChangeProperty(
                dpy, xid, typeProp, XA_ATOM, 32, PROP_MODE_REPLACE, longArrayOf(utility), 1,
            )
        }
        val stateProp = api.XInternAtom(dpy, ATOM_WM_STATE, 0)
        val skip = listOf(
            api.XInternAtom(dpy, ATOM_WM_STATE_SKIP_TASKBAR, 0),
            api.XInternAtom(dpy, ATOM_WM_STATE_SKIP_PAGER, 0),
        )
        val state = readAtoms(api, dpy, xid, stateProp) ?: return false
        val want = appendMissing(state, skip)
        if (want != state) {
            api.XChangeProperty(
                dpy, xid, stateProp, XA_ATOM, 32, PROP_MODE_REPLACE, want.toLongArray(), want.size,
            )
        }
        api.XFlush(dpy)
        return true
    }

    /** 找标题是我们的那条窗：BFS（WM reparent 后客户端窗在根之下 ≥2 层，深度给 4 层富余）。 */
    private fun findWindow(api: XTaskbarApi, dpy: Pointer, title: String): Long? {
        val nameAtom = api.XInternAtom(dpy, ATOM_WM_NAME, 1)
        if (nameAtom == 0L) return null // 服务端还没有人用过 _NET_WM_NAME ⇒ 没有窗能匹配
        val pidAtom = api.XInternAtom(dpy, ATOM_WM_PID, 1)
        val myPid = ProcessHandle.current().pid()
        val queue = ArrayDeque<Pair<Long, Int>>()
        queue += api.XDefaultRootWindow(dpy) to 0
        // ponytail: 预算 2000 防病态深树拖垮 1s 轮询；正常桌面全树就几百个窗。
        var budget = 2000
        while (queue.isNotEmpty() && budget-- > 0) {
            val (w, depth) = queue.removeFirst()
            val name = readString(api, dpy, w, nameAtom)
            if (name == title) {
                val pid = readAtoms(api, dpy, w, pidAtom)?.firstOrNull()
                if (windowMatches(name, pid, myPid, title)) return w
            }
            if (depth < 4) children(api, dpy, w).forEach { queue += it to depth + 1 }
        }
        return null
    }

    private fun children(api: XTaskbarApi, dpy: Pointer, w: Long): List<Long> {
        val root = LongByReference()
        val parent = LongByReference()
        val kids = PointerByReference()
        val n = IntByReference()
        if (api.XQueryTree(dpy, w, root, parent, kids, n) == 0) return emptyList()
        val p = kids.value ?: return emptyList()
        try {
            return (0 until n.value).map { p.getLong(it * 8L) } // Window = unsigned long
        } finally {
            api.XFree(p)
        }
    }

    /** 读一个 ATOM/CARDINAL 列表属性（缺属性 = 空表；协议层读失败 = null = 这条窗没了）。 */
    private fun readAtoms(api: XTaskbarApi, dpy: Pointer, xid: Long, prop: Long): List<Long>? {
        if (prop == 0L) return emptyList()
        val type = PointerByReference()
        val fmt = IntByReference()
        val n = LongByReference()
        val after = LongByReference()
        val data = PointerByReference()
        val rc = api.XGetWindowProperty(
            dpy, xid, prop, 0L, 64L, 0, ANY_PROPERTY_TYPE, type, fmt, n, after, data,
        )
        if (rc != 0) return null
        val p = data.value ?: return emptyList()
        try {
            // format 32 的数据按 long 数组返回（Xlib 口径），每项在低位。
            return (0 until n.value.toInt()).map { p.getLong(it * 8L) }
        } finally {
            api.XFree(p)
        }
    }

    private fun readString(api: XTaskbarApi, dpy: Pointer, xid: Long, prop: Long): String? {
        val type = PointerByReference()
        val fmt = IntByReference()
        val n = LongByReference()
        val after = LongByReference()
        val data = PointerByReference()
        val rc = api.XGetWindowProperty(
            dpy, xid, prop, 0L, 512L, 0, ANY_PROPERTY_TYPE, type, fmt, n, after, data,
        )
        if (rc != 0) return null
        val p = data.value ?: return null
        try {
            return if (fmt.value == 8 && n.value > 0) p.getString(0, "UTF-8") else null
        } finally {
            api.XFree(p)
        }
    }
}
