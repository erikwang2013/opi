// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 状态区外层入口验证驱动 —— **手工 harness，不进 CI**（前置条件与局限同
// opi_panel_driver.cpp：真 fcitx5 5.1.12 头/库 + 已构建的 Rust cdylib，
// 不需要 dbus；跑法见 run-harness.sh）。
//
// 为什么单独一个程序而不塞进面板驱动：那个文件加完 Shift/皮肤/简繁几段后已
// 489/500 行（两回越线都是它），而这里验的是**另一族事** —— 状态栏上「中/英」
// 「简/繁」「设」三个动作的挂载、点击与互相联动（「面板推进」一节不碰）。
// 拆开之后两个文件都离门禁有距离。
//
// 局限（如实记）：这里点「设」走的是**假启动器**（见 opi_status.h 的测试缝）——
// 本驱动证明「点击会发起启动、发起的是本插件那个候选」，不证明窗口真的起来
// （那要真桌面，见 README 的 dbus 限制）。

#include "opi_fcitx5.cpp"

#include <cstdio>
#include <string>

#include "opi_driver_common.h" // 测试器具（TestIC/check/…）

int main() {
    fcitx::InputContextManager manager;
    // IC 建在堆上且不释放 —— 同面板驱动（~InputContext 的 destroyed_ 断言）。
    TestIC &ic = *(new TestIC(manager));
    OpiEngine engine(nullptr); // instance_ 在胶水里只存不用
    fcitx::InputMethodEntry entry("opi", "OPI", "zh_CN", "opi_fcitx5");

    // activate 才挂状态项（core 在每次 activate 前清空 StatusGroup::InputMethod）。
    fcitx::InputContextEvent activateEvent(
        &ic, fcitx::EventType::InputContextSwitchInputMethod);
    engine.activate(entry, activateEvent);

    // 文案随**当前状态**变（中/英、简/繁），所以按两个字找同一枚动作。
    auto findAction = [&](const char *t1, const char *t2) -> fcitx::Action * {
        for (auto *a : ic.statusArea().allActions()) {
            const std::string t = a->shortText(&ic);
            if (t == t1 || (t2 != nullptr && t == t2)) {
                return a;
            }
        }
        return nullptr;
    };

    std::printf("== 1. 状态区三项都挂上了 ==\n");
    fcitx::Action *zhEn = findAction("中", "英");
    fcitx::Action *jianFan = findAction("简", "繁");
    fcitx::Action *settings = findAction("设", nullptr);
    check(zhEn != nullptr, "有「中/英」项");
    check(jianFan != nullptr, "有「简/繁」项");
    check(settings != nullptr, "有「设置」项");

    std::printf("== 2. 简/繁：点击切换 + 文案 ==\n");
    if (jianFan != nullptr && zhEn != nullptr) {
        jianFan->activate(&ic);
        check(opi_fcitx5_mode() == 4, "点「简」切到繁体（mode==4）");
        check(jianFan->shortText(&ic) == "繁", "文案变「繁」");
        check(zhEn->shortText(&ic) == "中", "「中/英」项不受影响（仍是「中」）");
        jianFan->activate(&ic);
        check(opi_fcitx5_mode() == 0 && jianFan->shortText(&ic) == "简",
              "再点切回简体（能进能出）");
    }

    std::printf("== 3. 交叉联动：英文态点「简」进繁体，「中/英」要刷回「中」 ==\n");
    if (zhEn != nullptr && jianFan != nullptr) {
        zhEn->activate(&ic); // 中 → 英
        check(opi_fcitx5_mode() == 1 && zhEn->shortText(&ic) == "英",
              "点「中」进英文");
        jianFan->activate(&ic); // 英 → 繁（与 Ctrl+Shift+F 同一条规则）
        check(opi_fcitx5_mode() == 4 && zhEn->shortText(&ic) == "中",
              "点「简」从英文直接进繁体，且「中/英」刷回「中」（两状态项联动）");
        check(jianFan->shortText(&ic) == "繁", "「简/繁」自身文案也是「繁」");
        jianFan->activate(&ic); // 回简体 —— 别把状态留给后面的用例
        check(opi_fcitx5_mode() == 0, "回到简体态");
    }

    std::printf("== 4. 设置项：点击走启动器（假货，不真拉 GUI）==\n");
    {
        int calls = 0;
        auto &hook = opiSettingsLauncherHook();
        auto real = hook;
        hook = [&calls]() {
            ++calls;
            return true;
        };
        if (settings != nullptr) {
            settings->activate(&ic);
        }
        check(calls == 1, "点「设」恰好调用启动器一次");
        hook = real;
    }
    // 候选表：前三个有效、越界是 null（查找规则是纯函数，这里钉住它）。
    check(opiSettingsAppCandidate(0) != nullptr &&
              opiSettingsAppCandidate(1) != nullptr &&
              opiSettingsAppCandidate(2) != nullptr &&
              opiSettingsAppCandidate(3) == nullptr,
          "启动候选表：三项有效、第 4 位越界为 null");

    std::printf("\n%s（失败 %d 项）\n", g_fail == 0 ? "ALL PASS" : "HAS FAILURES",
                g_fail);
    return g_fail == 0 ? 0 : 1;
}
