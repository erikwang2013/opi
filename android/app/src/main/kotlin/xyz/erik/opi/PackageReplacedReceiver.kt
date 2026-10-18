// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.os.Handler
import android.os.Looper
import android.os.Process
import android.util.Log
import xyz.erik.opi.engine.UserWordStore

/**
 * APK 覆盖安装后让新代码真正生效（`ACTION_MY_PACKAGE_REPLACED`）。
 *
 * 为什么只能结束进程：旧进程里 `System.loadLibrary` 装上的 .so 无法卸载，清
 * `inputViewCache` / 重载词库换的只是数据，跑的还是旧机器码。通常系统在覆盖安装时
 * 会自己杀掉进程；本接收器兜的是「旧进程被保留」的场景（部分 OEM / 安装器带
 * `INSTALL_DONT_KILL_APP`）。进程退出后，系统下次需要键盘时以新 APK 重新绑定服务。
 *
 * 时序：先让本进程所有用户词 store 把防抖中的保存落盘（[UserWordStore.flushAll]），
 * 再延迟 [EXIT_DELAY_MS] 退出 —— 覆盖 [UserWordStore.SAVE_DEBOUNCE_MS] 的窗口，
 * 也给落盘线程写完文件的时间。
 *
 * ⚠️ 类名别改：旧进程可能按新 manifest 里的名字解析本类，而它加载的是旧 APK 的
 * 类加载器。改名只会让「从含本类之前的版本升上来」的那一次失去兜底（那一版旧 APK
 * 里本来也没有本类，无法兜底；之后保持名字稳定才有意义）。
 *
 * 真机限制见 docs/install/android.md §3.4（OEM 可能不投递/延迟投递本广播）。
 */
class PackageReplacedReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (!handles(intent.action)) return
        Log.i(TAG, "MY_PACKAGE_REPLACED: flush user words, exit in ${EXIT_DELAY_MS}ms")
        UserWordStore.flushAll()
        // 不用 goAsync：进程若正被系统绑定（IME），不会因 onReceive 返回而被冻结。
        Handler(Looper.getMainLooper()).postDelayed({
            Process.killProcess(Process.myPid())
        }, EXIT_DELAY_MS)
    }

    companion object {
        private const val TAG = "PackageReplaced"

        /** 防抖窗口 + 500ms 余量（用户词文件只有几 KB，落盘线程通常毫秒级完成）。 */
        internal const val EXIT_DELAY_MS = UserWordStore.SAVE_DEBOUNCE_MS + 500

        /** 纯判定（JVM 可测）：只认覆盖安装广播。 */
        internal fun handles(action: String?): Boolean =
            action == Intent.ACTION_MY_PACKAGE_REPLACED
    }
}
