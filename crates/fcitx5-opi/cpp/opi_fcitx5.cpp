// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// OPI fcitx5 插件胶水（B2 计划缺口）：Rust 逻辑出口（libfcitx5_opi.so 的
// C 符号）与 fcitx5 5.1.x AddonInstance/InputMethod 之间的薄层。
//
// 行为路由全部在 Rust 侧（crates/fcitx5-opi/src/input_method.rs，镜像
// Android KeyRouter），本文件只做：键事件 → opi_fcitx5_key_event →
// 按动作码（0=直通 1=已处理 2=提交）行事。字符串约定 UTF-8+长度，
// 返回值由 Rust 侧分配，用 opi_ffi_free_string_utf8 释放。
//
// 构建与验证方式见 README.md（头文件经 apt-get download 解包，无需 root）。

#include <fcitx/addonfactory.h>
#include <fcitx/addoninstance.h>
#include <fcitx/addonmanager.h>
#include <fcitx/candidatelist.h> // CommonCandidateList / CandidateWord
#include <fcitx/event.h> // KeyEvent / KeyEventBase。fcitx5 5.1.x 无 fcitx/keyevent.h
#include <fcitx/inputcontext.h>
#include <fcitx/inputmethodengine.h>
#include <fcitx/inputpanel.h> // setPreedit / setCandidateList
#include <fcitx/instance.h>
#include <fcitx/text.h>
#include <fcitx/userinterface.h> // UserInterfaceComponent
#include <fcitx-utils/event.h> // 用户词落盘定时器（EventLoop::addTimeEvent / now）
#include <fcitx-utils/standardpath.h>

#include "opi_json.h"     // parseJsonStringArray（拆出去是为了本文件守住 500 行硬规矩）
#include "opi_config.h"  // 「属性设置」的六个选项（同样为了 500 行门禁拆出去）
#include "opi_engine.h"  // addon 类声明（opi_mode.h / opi_skin.h 由它带入）
#include "opi_mode.h"    // 模式热键 + Shift 轻按（2026-10-09 从本文件搬出去）
#include "opi_rust_ffi.h" // Rust 出口声明 + take()，同一次拆分

#include <cstdint>
#include <cstdio>
#include <exception>
#include <memory>
#include <string>
#include <utility>
#include <vector>

// ---------- 输入面板：预编辑 + 候选栏 ----------
//
// ⚠️ UI 通路**不在** AddonInstance 的虚表里。setPreedit / setCandidateList 是
// InputPanel 的方法（fcitx/inputpanel.h:55 / :68），updateUserInterface 是
// InputContext 的方法（fcitx/inputcontext.h:210）。引擎要**主动调用**它们，
// 没有「少 override 了哪个虚函数」这回事 —— 原码的问题是这些函数一次都没被
// 调用过（`InputMethodEngine` 里确实没有对应的虚函数可 override）。
//
// 本文件只推**当前页**给前端：Rust 侧 opi_fcitx5_candidates 返回的已经是切片后
// 的当前页，翻页由 Rust 路由驱动（input_method.rs 的 PageUp/PageDown）。若把整份
// 候选交给 CommonCandidateList 自己翻，C++ 与 Rust 就各存一份页码，前端点翻页
// 箭头时两边立刻漂移。
//
// 取回候选的条数上限：**不要**再往这里写一个「Rust PAGE_SIZE 的镜像」。
// Rust 的 `opi_fcitx5_candidates(limit)` 返回的已经是**切好的整页**
// （`candidate.rs` 的 `candidates()` 按它自己的 PAGE_SIZE `.skip().take()`，
// 这里的 limit 是在那之后再切一刀），所以本文件根本不需要知道页大小；
// 页大小由**取回的条数**决定（见 refreshingPanelFor）。
// 给 SIZE_MAX = 「不切」：整页照单全收，页大小自然跟随 Rust，Rust 改 8→9
// 这边不用动，也不会少画。
static constexpr size_t kOpiCandidateFetchLimit = SIZE_MAX;

// 上一次推给前端的面板预编辑串。唯一用途：直通键（action==0）不消费按键、
// Rust 侧状态不变，原样重推要白付一次跨 FFI + 候选 JSON + 客户端重绘，而
// ESC/方向键/F5 每次按键都来一遍 —— 缓冲相同就跳过（见 keyEvent 尾部）。
// 进程内一个静态量够用：Rust 侧状态本就是进程单例，面板只是它的镜子；切
// 输入上下文时 fcitx5 会调 reset()，那里两边一起清空并刷新这个缓存。
static std::string g_pushedBuffer;
// 上一次推给前端时是否有候选。与 g_pushedBuffer 合起来判「联想态」（缓冲空 +
// 有候选）：联想没有预编辑，单看缓冲会把它误当成空闲（Spec §5，2026-10-27）。
static bool g_pushedHadCandidates = false;
// 「智能联想」开关（配置值在 C++ 侧消费）：false = 空缓冲候选不展示，且引擎
// 建出的联想立即清掉。由 opi_config.h 的 applyOpiSettings 冷路径写入。
static bool g_associationEnabled = false;

void opiApplyAssociationSetting(bool on) { g_associationEnabled = on; }

// 候选词：点选后走 Rust 侧 select(**页内**索引) 并提交。
// CandidateWord::select 是纯虚（fcitx/candidatelist.h:44），必须实现。
class OpiCandidateWord : public fcitx::CandidateWord {
public:
    OpiCandidateWord(std::string text, size_t index)
        : fcitx::CandidateWord(fcitx::Text(std::move(text))), index_(index) {}
    void select(fcitx::InputContext *ic) const override;

private:
    // 页内索引（与 Rust 侧 CandidateState::select 同语义 = page*PAGE_SIZE+index）。
    size_t index_;
};

// 把 Rust 侧当前状态（缓冲 = 预编辑串，当前页候选）推到 fcitx5 面板。
// 定义在下面；声明在 opi_engine.h（类声明搬走后它不能再是文件级 static）。
void refreshingPanelFor(fcitx::InputContext *ic);

void OpiCandidateWord::select(fcitx::InputContext *ic) const {
    // 点选是从 UI 线程调进来的第三个入口，同样要挡异常（见 keyEvent 的说明）。
    try {
        const std::string out = take(opi_fcitx5_select(index_));
        if (ic != nullptr && !out.empty()) {
            ic->commitString(out);
        }
        // 选完 Rust 侧缓冲/候选都变了，面板必须同步 —— 否则候选栏会留在屏幕上。
        refreshingPanelFor(ic);
    } catch (const std::exception &e) {
        fprintf(stderr, "fcitx5-opi: select 异常，本次点选作废: %s\n", e.what());
    } catch (...) {
        // 非 std::exception 的异常若逃进 fcitx5 主循环会带走整个守护进程（语言栏一起没）。
        fprintf(stderr, "fcitx5-opi: select 未知异常，本次点选作废\n");
    }
}

// 普通候选与联想候选共用（2026-10-28）：页大小 = 实际推过去的条数 → 候选表恒为
// 单页；选择键 1..n 让 fcitx5 画序号并把数字路由到 OpiCandidateWord::select
// （**页内索引**，与 Rust 路由同语义）。空表不设 pageSize / 高亮位
// （setGlobalCursorIndex(0) 对空表抛 invalid_argument）。
static std::unique_ptr<fcitx::CommonCandidateList> makeCandidateList(
    const std::vector<std::string> &texts) {
    auto list = std::make_unique<fcitx::CommonCandidateList>();
    if (texts.empty()) {
        return list;
    }
    list->setPageSize(static_cast<int>(texts.size()));
    for (size_t i = 0; i < texts.size(); ++i) {
        list->append<OpiCandidateWord>(texts[i], i);
    }
    fcitx::KeyList selectionKeys;
    for (size_t i = 0; i < texts.size(); ++i) {
        selectionKeys.push_back(fcitx::Key(std::to_string(i + 1)));
    }
    list->setSelectionKey(selectionKeys);
    list->setGlobalCursorIndex(0);
    return list;
}

void refreshingPanelFor(fcitx::InputContext *ic) {
    if (ic == nullptr) {
        return;
    }
    auto &panel = ic->inputPanel();
    const std::string buffer = take(opi_fcitx5_buffer());
    // 候选**只取一次**（2026-10-27）：联想判据要用它，下面的正常分支也复用它；
    // 重复调 opi_fcitx5_candidates 会把 Rust 侧的页游标推进两次。
    const auto texts = parseJsonStringArray(
        take(opi_fcitx5_candidates(kOpiCandidateFetchLimit)));
    // 客户端声明了 Preedit 才走内联预编辑通道；两个入口共用这一个判断。
    const bool wantsClientPreedit =
        ic->capabilityFlags().test(fcitx::CapabilityFlag::Preedit);
    // 智能联想默认关：空缓冲 + 有候选 = 联想项（Spec §5）。关闭时不展示，
    // 并立即清掉引擎里的联想 —— 否则数字键/链式会在看不见的状态下生效。
    const bool associationHidden = buffer.empty() && !texts.empty() && !g_associationEnabled;
    if (associationHidden) {
        opi_fcitx5_clear();
        panel.reset();
    } else if (buffer.empty() && texts.empty()) {
        // 缓冲空 = 本引擎当前无 composing。用 reset() 而不是 setPreedit("")：
        // 它把预编辑、候选表、辅助串一起清干净（含 clientPreedit），正是
        // 「无 composing」的完整表达。
        panel.reset();
    } else if (buffer.empty()) {
        // 空缓冲 + 有候选 = 联想态（Spec §5，2026-10-27）：只建候选表。
        // **不设** preedit / clientPreedit —— 联想不是 composing；候选序号与
        // 数字选词由 makeCandidateList 统一给（2026-10-28 用户报障）。
        panel.reset();
        panel.setCandidateList(makeCandidateList(texts));
    } else {
        fcitx::Text preedit(buffer);
        // 光标在预编辑串末尾（按字节）。2026-10-17 起缓冲可能带**逐字确认的汉字**
        // （display_buffer 前置 pending，UTF-8 多字节）—— 这里的 size() 是字节长度，
        // 与各引擎 setCursor(串字节长) 的惯例一致（光标落在末尾）。
        preedit.setCursor(static_cast<int>(buffer.size()));
        panel.setPreedit(preedit);
        // clientPreedit 是**另一条通道**，不是面板 preedit 的别名：
        //   ClientSideInputPanel(1<<39) → 整个面板（UpdateClientSideUI）
        //   Preedit(1<<1)               → 内联预编辑（UpdateFormattedPreedit）
        // 只推前者，声明了 Preedit 的客户端**全程看不到内联预编辑**，且切窗口时
        // 已输入的拼音会**静默丢失** —— core 对「有 clientPreedit 的客户端失焦」
        // 有默认提交行为（实测：只加下面三行的变异体在 FocusOut() 时收到
        // `CommitString: 'ni'`，原码 0 条）。
        // 守卫按能力位。注意它**不改变客户端的可观测结果**：core 的 updatePreedit()
        // 自己就查这一位（去掉守卫后 updatePreeditImpl 仍被调 0 次，DBus 上
        // UpdateFormattedPreedit 条数也不变）。真正决定客户端收到什么的是 core。
        if (wantsClientPreedit) {
            panel.setClientPreedit(preedit);
        }

        // texts 已在函数顶部取过（一次抓取；重复调会推进 Rust 页游标）。
        // 候选序号/选择键/高亮位的理由见 makeCandidateList 上方。
        panel.setCandidateList(makeCandidateList(texts));
    }
    // 清空也要发：提交/失焦后不通知，客户端光标处会一直挂着上次的内联预编辑。
    if (wantsClientPreedit) {
        ic->updatePreedit();
    }
    // 通知前端刷新。immediate=false（默认）：fcitx5 会把同一轮事件里的多次
    // 更新合并成一次 UI 刷新，这是引擎该走的路。
    ic->updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
    // 推完了才记 —— 半路抛异常时这里保持旧值，下次直通键会照常重推一遍。
    g_pushedBuffer = buffer;
    g_pushedHadCandidates = !texts.empty() && !associationHidden;
}

// ---------- W1「直接确认」：切走/切英文时把待定输入提交掉（2026-10-10） ----------
//
// 用户裁决：焦点移出**不提交**（静默清空）；但**切输入法**与**切英文**要把用户已经
// 打了一半的拼音提交出去，别丢输入 —— **按引擎当前模式分叉**：
//   · 拼音/繁体（字母缓冲）⇒ 提交**缓冲原文**；
//   · 符号模式（`、,` 关键字缓冲）⇒ **不上关键字**：只清缓冲、提交空串
//     （关键字不是待上屏文本，塞进文档比丢缓冲更坏 —— 同夜追加裁决；用户说的
//     「默认是拼音」就是拼音场景）。
//
// ⚠️ 语义订正（时间线，别照旧注释写）：
//   · 2026-10-10 白天（lead 裁决）：有候选上首候选、无候选提交原文；
//   · 2026-10-10 深夜（**用户裁决「切英文 = 默认是拼音，不要是候选词」**）：桌面
//     与移动端统一 —— 一律原文。移动端先改并实证（[英文] → 字段收 `ni'hao`
//     不是「你好」），桌面这侧把 select(0) 首候选那一档去掉；
//   · 同夜追加（coordinator 转裁决）：**按模式分叉** —— 「一律原文」只对字母缓冲
//     成立，符号模式的关键字不上屏（engine-track 引擎侧同一分叉已同步）。
//   「原文」不是「去查候选表再决定」：原文与候选面无关，少一个会与 Rust 侧
//   漂的判据（此前那版拿 select(0) 的空串当「有没有候选」，本身是隐式分档）。
//   分叉判据用**引擎当前 mode**（opi_fcitx5_mode()，FFI 已有，无需新读口）；
//   读的是 ① 退浏览**之后**的 mode（浏览退出时真缓冲/模式一起恢复）。
//
// 顺序契约（lead 2026-10-10，且与符号浏览有关）：**先退浏览、后提交** ——
// 符号浏览进行中时先让引擎退出浏览（恢复浏览前的真缓冲），再按 W1 提交。反过来的话
// 提交出去的是浏览态的标题串（或首个分类），那就成了「按一下 Shift 多出一串分类名」。
//
// 缓冲也是空的（没打过字 / 已是英文模式）就什么都不提交 —— commitString("") 没有
// 意义，空串正好兼作「没东西可提交」的判据。
void opiDirectConfirm(fcitx::InputContext *ic) {
    if (ic == nullptr) {
        return;
    }
    // ① 先退浏览（幂等；引擎侧入口未接线时是 no-op，见 opi_mode.h 的 opiSymbolBrowseCall）
    opiSymbolBrowseHook()(kOpiSymbolBrowseExit);
    // ② 符号模式：关键字不上屏 —— 清干净就走（缓冲区那串是 `;sh` 之类关键字，
    //    不是用户想写进文档的字）。清的是引擎缓冲/候选/预编辑，与下面同一套。
    if (opi_fcitx5_mode() == kModeSymbol) {
        opi_fcitx5_clear();
        return;
    }
    // ③ 拼音/繁体：一律原文 —— 真缓冲提交（与 typed 逐字一致，不带候选、不加分隔符）。
    // 显示出口 2026-10-23 起全键盘带分词（`ni'hao`）；老 .so 没有原文出口，而那时
    // `opi_fcitx5_buffer` 本来就返回原文，退回它（两种 .so 都提交 typed 逐字）。
    std::string out = opi_fcitx5_raw_buffer != nullptr
                          ? take(opi_fcitx5_raw_buffer())
                          : take(opi_fcitx5_buffer());
    if (!out.empty()) {
        // 原文已经交出去了，引擎状态要跟着清（缓冲/候选/预编辑），否则预编辑串
        // 会留在屏幕上、候选也会错位到下一段输入。
        opi_fcitx5_clear();
        ic->commitString(out);
    }
}

// ---------- 模式热键与 Shift 轻按 → 已整体搬去 opi_mode.h ----------
// isRepeatEvent / OpiMode 编码 / handleModeHotkey / OpiShiftToggle 的**唯一家**是
// opi_mode.h（2026-10-09 搬移，理由：本文件 500 行门禁）。搬移是机械的，判据与
// 实测记录（键位占用表、REPEAT 实测、与 tsf-opi/src/vk.rs 的同构关系）随注释走。
// 本文件只留：全角键（下一段）、按 Rust 路由的骨架、以及 keyEventImpl 里的接线。

// ---------- 全角 ⇄ 半角切换键（用户裁决 2026-09-27） ----------

// Shift+Space。与模式热键**同一条理由**判在 Rust 路由之前：Shift+Space 送进去
// 就是普通空格（input_method.rs 的 KEY_SPACE 分支不看 Shift 位），会被当成
// 「选首候选 / 提交缓冲」—— 进来就出不来了。
//
// 键位证据（2026-09-27 实测，方法同 handleModeHotkey 的占用表）：
// `strings` 扫全部 fcitx5 库的 `^(Shift|Control|Super|Alt)\+` 默认组合，占用为
// Control+7 / Control+8 / Control+period / Control+semicolon / Control+Return
// （±Shift、±KP_Enter）/ Control+Shift+U / Control+Alt+E / Control+Alt+Shift+U /
// Shift+Tab / Super+grave / Super+semicolon —— **Shift+Space 不在其中**；
// /usr/share/fcitx5/addon/*.conf 与 ~/.config/fcitx5/conf/*.conf 里也搜不到
// `shift+space`（pinyin/table 只把它作为 addon 依赖名 `fullwidth` 提到，不是键位）。
// 生态约定：Shift+Space 本就是 CJK 输入法的「全角空格」，这里借它当**开关**。
// ⚠️ 本机没有 Windows ⇒ TSF 轨同键（VK_SPACE + Shift）**未验证**，见 vk.rs。
//
// 带 Ctrl/Alt 的不算：Ctrl+Space 是 fcitx5 的输入法切换键、Ctrl+Shift+Space 是
// 它的反向键 —— 实测在 ~/.config/fcitx5/config 的 `[Hotkey/TriggerKeys]` 与
// `[Hotkey/AltTriggerKeys]`（**全局热键，先于输入法**，故 strings 表里搜不到它们：
// 那是配置值不是库内字符串）；Alt+Space 在多数桌面环境是窗口菜单。
// 收窄条件避免与它们撞。
// 返回 true = 本键已被本插件消费。
static bool handleFullwidthHotkey(fcitx::KeyEvent &keyEvent) {
    const fcitx::Key key = keyEvent.key();
    const auto st = key.states(); // KeyStates(Flags<KeyState>)；不写类型名以免与别名漂
    if (!st.test(fcitx::KeyState::Shift) || st.test(fcitx::KeyState::Ctrl)
        || st.test(fcitx::KeyState::Alt)) {
        return false;
    }
    if (key.sym() != FcitxKey_space) {
        return false;
    }
    if (!keyEvent.isRelease() && !isRepeatEvent(keyEvent)) {
        opi_fcitx5_toggle_fullwidth();
        // **不刷面板**：这个开关只改后续的标点映射，不动缓冲也不动候选
        // （Rust 侧 Engine::toggle_fullwidth 只翻一个 bool），推一帧是白推。
        // 与 handleModeHotkey 的 refreshingPanelFor 差别就在这里 —— 那边切模式
        // 会清缓冲换候选，不刷会留着上一次的候选。
    }
    // 抬起**与重复**一并消费、但不再切（同 handleModeHotkey：只拦按下会给客户端
    // 一个无 keydown 的 keyup；重复放行更糟 —— 会被引擎当普通空格去选首候选）。
    keyEvent.filterAndAccept();
    return true;
}

// ---------- fcitx5 插件本体（结构对齐 fcitx5 example/ime.cpp） ----------
// （opi_engine.h 的 include 已提到文件头部：opi_mode.h 的 inline 函数要在本文件
// 的中间段落就可见。）

// Rust 侧线格式占用的三位（../src/input_method.rs 的 KEY_STATE_RELEASED/
// KEY_STATE_REPEAT/KEY_STATE_LONG_PRESSED = 1<<26 / 1<<27 / 1<<28）。
// 这三位的编号不是凭空选的：fcitx5 自己的 KeyState 在 24..31 段另有含义 ——
// fcitx-utils/keysym.h:22
//   enum class KeyState : uint32_t {
//       ...
//       HandledMask = 1U << 24,  IgnoredMask = 1U << 25,
//       Super2      = 1U << 26,  // Gtk virtual Super
//       Hyper2      = 1U << 27,  // Gtk virtual Hyper
//       Meta        = 1U << 28,
//       Virtual     = 1U << 29,  Repeat = 1U << 31, UsedMask = 0x5c001fff,
//   };
// 即 1<<26/27/28 分别是 Super2/Hyper2/Meta，与我们的线格式撞号。一旦这三个位
// 混进送往 Rust 的值，按住虚拟 Super 的普通字母会被读成「抬起」
// （input_method.rs:84 `states & KEY_STATE_RELEASED`）而整键吞掉。
// 故 OR 之前先掩掉：这两个域不能共用一个 u32 而不设边界。
//
// 实测边界（链接 libFcitx5Utils 5.1.12 跑 Key::normalize()，非读头文件转述）：
//   Super2 in=0x04000000 -> 0x40（降级成普通 Super）   Hyper2/Meta/Repeat -> 0
//   HandledMask/IgnoredMask/Virtual/CapsLock/Mod5 -> 全 0
// 即 normalize() 只留 Ctrl/Alt/Shift/Super，撞号位在下一行 `key().states()`
// 那一步已经被滤掉 —— 所以本掩码当前是**防御性**的，不是在修一个会现场触发的
// 活 bug（原码调用的 keyEvent.states() 根本不存在，那段 OR 从未运行过）。
// 它挡的是回归：真 API 里 rawKey()/origKey() 返回**未归一化**状态，而下面为了
// 取 Repeat 必须用 rawKey() —— 谁把基底换成 rawKey().states() 谁就需要这个掩码。
static constexpr uint32_t kOpiWireMask = (1u << 26) | (1u << 27) | (1u << 28);

// 把「撞号」这件事钉在编译期：fcitx5 若哪天挪走 Super2/Hyper2/Meta 的位置，
// 或有人改了上面的掩码，这里先炸，而不是等桌面端出现「按键被吞」再查。
static_assert(static_cast<uint32_t>(fcitx::KeyState::Super2) == (1u << 26), "Super2 挪位了：更新 kOpiWireMask 与 Rust 线格式");
static_assert(static_cast<uint32_t>(fcitx::KeyState::Hyper2) == (1u << 27), "Hyper2 挪位了：更新 kOpiWireMask 与 Rust 线格式");
static_assert(static_cast<uint32_t>(fcitx::KeyState::Meta) == (1u << 28), "Meta 挪位了：更新 kOpiWireMask 与 Rust 线格式");

void OpiEngine::keyEvent(const fcitx::InputMethodEntry &entry,
                         fcitx::KeyEvent &keyEvent) {
    try {
        keyEventImpl(entry, keyEvent);
    } catch (const std::exception &e) {
        fprintf(stderr, "fcitx5-opi: keyEvent 异常，本次按键作废: %s\n", e.what());
    } catch (...) {
        fprintf(stderr, "fcitx5-opi: keyEvent 未知异常，本次按键作废\n");
    }
}

void OpiEngine::keyEventImpl(const fcitx::InputMethodEntry & /*entry*/,
                             fcitx::KeyEvent &keyEvent) {
    // 联想态（Spec §5，2026-10-27）：**按下**先收起再执行本键 —— 无论它随后是
    // 热键、直通还是普通输入，都不该让联想候选活过这一步。候选窗没有预编辑，
    // 只能按「上次推出去的状态」判：缓冲空 + 有候选。抬起不动作。
    // 例外（2026-10-28 用户报障）：数字选联想项、PageUp/PageDown 与 `-`/`=`
    // 翻联想页 —— 先收起会让这些键永远落空；它们交给 Rust 路由处理。
    const uint32_t keySym = keyEvent.key().sym();
    const bool associationKey = g_associationEnabled
        && ((keySym >= '1' && keySym <= '9') || keySym == 0xff55
            || keySym == 0xff56 || keySym == '-' || keySym == '=');
    const bool associationShown = !keyEvent.isRelease()
        && g_pushedBuffer.empty() && g_pushedHadCandidates && !associationKey;
    if (associationShown) {
        opi_fcitx5_clear();
        refreshingPanelFor(keyEvent.inputContext());
    }
    // Shift 轻按判定**最先**：它要看每一个按下事件来撤销判定（Shift+字母/
    // 方向/空格都不是轻按），且从不消费事件、不碰 Rust 状态（见 opi_mode.h）。
    shiftToggle_.handle(keyEvent, *settings_.shiftToggle);
    // 模式热键**先于** Rust 路由：Ctrl 组合键在 input_method.rs 首行就被直通，
    // 送进去也是白送（见 opi_mode.h 的说明）。
    if (handleModeHotkey(keyEvent, shiftToggle_, fanjianToggle_)) {
        return;
    }
    // 全角键同样先于 Rust 路由，理由见 handleFullwidthHotkey 上方。
    if (handleFullwidthHotkey(keyEvent)) {
        return;
    }
    // keyval 取 xkb keysym（ASCII 段与 Unicode 码点一致）；states 用
    // KeyEventBase::key() 的 KeyStates，再补全 Rust 侧线格式的三位。
    //
    // 取 key()（normalize 后的 Key）而非 rawKey()：key() 已把 states 收敛到
    // ctrl/alt/shift/super（fcitx-utils/key.h:157），值域小且与前端无关。
    uint32_t states = static_cast<uint32_t>(keyEvent.key().states()) & ~kOpiWireMask;
    if (keyEvent.isRelease()) { // KeyEventBase::isRelease()，fcitx/event.h:325
        states |= 1u << 26;
    }
    // 重复位取自 rawKey：KeyState::Repeat = 1U<<31 是前端置的原始位，而 key()
    // 经 normalize() 只保留 ctrl/alt/shift/super，重复位在那一步已被滤掉。
    if (keyEvent.rawKey().states() & fcitx::KeyState::Repeat) {
        states |= 1u << 27;
    }
    // 长按（1<<28）在 fcitx5 5.1.x 的 KeyEvent 上没有对应源：唯一的长按概念在
    // VirtualKeyboardEvent::isLongPress()（fcitx/event.h:387，触屏虚拟键盘专用）。
    // 此处不合成该位 —— 恒为 0，Rust 侧 handle_shift 的长按分支（→
    // shift_long_press）在 fcitx5 桌面端不可达，需 B3 另行决定入口。见 README。
    const OpiKeyEventResult result = opi_fcitx5_key_event(keyEvent.key().sym(), states);
    switch (result.action) {
    case 2: // Commit：提交文本并消费按键
        // commitString 是 InputContext 的成员（inputcontext.h:177），不是引擎的。
        if (auto *ic = keyEvent.inputContext()) {
            ic->commitString(take(result.text));
        }
        keyEvent.filterAndAccept();
        break;
    case 1: // EngineHandled：已消费，不再转发
        keyEvent.filterAndAccept();
        break;
    default: // 0 PassThrough：不拦截，交客户端应用处理
        break;
    }
    if (result.action != 2) {
        opi_ffi_free_string_utf8(result.text);
    }
    // 抬起事件不改引擎状态：路由的 released 分支只回放按下时记下的结论
    // （last_printable），缓冲/候选/模式/页码全不动（门禁：
    // input_method_release_tests.rs 的 release_never_mutates_engine_state）。
    // 旧实现照常重推面板 ⇒ 每个物理按键把全量候选排序付两遍；这里直接返回。
    // action==2 例外：抬头若将来出现「抬起提交」，提交后面板仍必须收起来。
    if (keyEvent.isRelease() && result.action != 2) {
        return;
    }
    // 直通（action==0）：Rust 侧没消费这个键、状态没动，重推的是同一份内容 ——
    // 幂等，但白花一次跨 FFI + 候选 JSON 序列化/解析 + 客户端重绘，而 ESC /
    // 方向键 / F5 / PageDown 这类直通键是**每次按键**都来一遍。缓冲与上次推出
    // 去的一样就跳过（reset()/select() 不经过这里，各自已经推过）。
    // 2026-10-26：Shift 按下也不改变缓冲/候选（Rust set_shift 只翻 bool），不重新推面板；
    // 旧实现每次 Shift 按下都付一次全量候选排序。
    const bool shiftOnly = !keyEvent.isRelease()
        && (keyEvent.key().sym() == FcitxKey_Shift_L
            || keyEvent.key().sym() == FcitxKey_Shift_R);
    if ((result.action == 0 || shiftOnly) && !associationShown
        && take(opi_fcitx5_buffer()) == g_pushedBuffer) {
        return;
    }
    // 把引擎新状态推给前端：预编辑串（缓冲）+ 当前页候选。
    refreshingPanelFor(keyEvent.inputContext());
}

// ---------- 用户词自动落盘 ----------
//
// Rust 侧在成功选词后标脏，并做 800ms 尾随防抖；本层只提供心跳：事件循环每 250ms
// 问一次「到点没」，到点才真正写文件。不在 Rust 起后台线程 —— addon 的生命周期与
// 事件循环都在宿主手里，析构时能停掉定时器并强制 flush 最后一次（<800ms 的尾巴）。
static std::unique_ptr<fcitx::EventSourceTime> g_userWordsTimer;

void OpiEngine::startUserWordsTimer() {
    // 手工 harness 的驱动用 `OpiEngine(nullptr)` 直接构造（instance_ 不入库、只存
    // 不用）；没有这个空判，2026-10-22 加的计时器会在驱动里空指针崩溃 —— 与联想
    // 无关的既存问题，2026-10-27 跑 Task 17 harness 时暴露。
    if (instance_ == nullptr || opi_fcitx5_save_user_words_if_due == nullptr) {
        return; // 旧 .so 没有该出口：保持「无自动落盘」，别让插件加载失败。
    }
    g_userWordsTimer = instance_->eventLoop().addTimeEvent(
        CLOCK_MONOTONIC, fcitx::now(CLOCK_MONOTONIC) + 250000, 0,
        [](fcitx::EventSourceTime *time, uint64_t) {
            opi_fcitx5_save_user_words_if_due();
            time->setNextInterval(250000); // 绝对时刻 = now + 250ms
            time->setOneShot();            // 重新武装一次性定时器（fcitx5 惯例）
            return true;
        });
}

OpiEngine::~OpiEngine() {
    if (opi_fcitx5_flush_user_words != nullptr) {
        opi_fcitx5_flush_user_words();
    }
    g_userWordsTimer.reset(); // 事件循环销毁前必须摘掉定时器
}

// 工厂类：FCITX_ADDON_FACTORY 宏（fcitx/addoninstance.h:193）只做
//   static ClassName factory; return &factory;
// 即它要求 ClassName 这个类型已经存在，自己并不定义 —— 上一版只写了宏调用而
// 仓库里从没有 OpiEngineFactory 的定义，那是编译错误的另一来源。
class OpiEngineFactory : public fcitx::AddonFactory {
public:
    fcitx::AddonInstance *create(fcitx::AddonManager *manager) override {
        return new OpiEngine(manager->instance());
    }
};

FCITX_ADDON_FACTORY(OpiEngineFactory);
