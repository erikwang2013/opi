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
#include <fcitx-utils/standardpath.h>

#include "opi_json.h"     // parseJsonStringArray（拆出去是为了本文件守住 500 行硬规矩）
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
static void refreshingPanelFor(fcitx::InputContext *ic);

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
    }
}

static void refreshingPanelFor(fcitx::InputContext *ic) {
    if (ic == nullptr) {
        return;
    }
    auto &panel = ic->inputPanel();
    const std::string buffer = take(opi_fcitx5_buffer());
    // 客户端声明了 Preedit 才走内联预编辑通道；两个入口共用这一个判断。
    const bool wantsClientPreedit =
        ic->capabilityFlags().test(fcitx::CapabilityFlag::Preedit);
    if (buffer.empty()) {
        // 缓冲空 = 本引擎当前无 composing。用 reset() 而不是 setPreedit("")：
        // 它把预编辑、候选表、辅助串一起清干净（含 clientPreedit），正是
        // 「无 composing」的完整表达。
        panel.reset();
    } else {
        fcitx::Text preedit(buffer);
        // 光标在预编辑串末尾（按字节；拼音缓冲是 ASCII，字节序即字符序）。
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

        auto list = std::make_unique<fcitx::CommonCandidateList>();
        const auto texts = parseJsonStringArray(
            take(opi_fcitx5_candidates(kOpiCandidateFetchLimit)));
        // 页大小 = **实际推过去的条数** → 候选表恒为单页（见文件头说明），
        // 且不需要任何「和 Rust 对齐」的常量：Rust 页变大，这里跟着变大。
        // 空表不设：页大小 0 不是合法值，而空表本来也没东西可翻。
        if (!texts.empty()) {
            list->setPageSize(static_cast<int>(texts.size()));
        }
        for (size_t i = 0; i < texts.size(); ++i) {
            list->append<OpiCandidateWord>(texts[i], i);
        }
        // 候选栏高亮位（-1 = 无高亮，前端不画选中态）。
        // ⚠️ 空判是**必须**的：对**空**候选表调 setGlobalCursorIndex(0) 会抛
        // `std::invalid_argument`（"CommonCandidateList: invalid global index"），
        // 而「缓冲非空 + 候选 0 条」是常态（`v`/`xz`/`zzzz` 都没候选）。抛出后：
        // 有 try/catch 时该键作废、面板停在上一次的候选表上；没有时 → abort。
        if (!texts.empty()) {
            list->setGlobalCursorIndex(0);
        }
        panel.setCandidateList(std::move(list));
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
}

// ---------- 模式热键（B0/B5） ----------
// 两个热键共用：这是按住不放补发的**重复**事件吗。**只能从 rawKey() 取** ——
// key() 已过 Key::normalize()，Repeat 在那一步被滤掉（下面送 Rust 的 states 处有实测），
// 拿 key().states() 判重复恒为 false。要排：热键是**切换**语义，按住时系统持续补发
// 重复 ⇒ 不排就是模式疯狂来回切 / 全角位疯狂翻。要**认领但不动作**而不能放行：放行的
// Shift+Space 会撞上 handleFullwidthHotkey 注释里那个 `KEY_SPACE` 坑（**不看 Shift 位**
// ⇒ 被当成「选首候选」），Ctrl+' 的重复则会直通到应用（抬手已被我们吃掉）。同判见
// tsf-opi/src/vk.rs 的 `hotkey_should_act`（两轨同构，改一处改两处）。
static bool isRepeatEvent(const fcitx::KeyEvent &keyEvent) {
    return keyEvent.rawKey().states().test(fcitx::KeyState::Repeat);
}

// 模式编码与 Rust 侧 mode_from_int / mode_to_int **逐值对应**（../src/lib.rs）。
// 抄错一个值就是切到另一个模式，且两边都不报错。
enum OpiMode : int32_t {
    kModePinyin = 0,
    kModeEnglish = 1,
    kModeSymbol = 3,
};

// ⚠️ 这段路由**故意落在 C++ 侧**，与本文件开头「行为路由全部在 Rust 侧」相悖。
// 原因很具体：Rust 侧拦 Ctrl 的是 handle_key 的首行
// （src/input_method.rs：`states & (CTRL|ALT) → PassThrough`），Ctrl 组合键根本
// 到不了 `opi_fcitx5_key_event` 的动作分发，只能在**送进去之前**拦。那个文件当前
// 有别的改动在飞（B 系列符号模式本体），不动它。长期归属是 input_method.rs ——
// 那里还能顺手覆盖 TSF 一侧，届时本段整体删除，改由 Rust 侧自己报模式变化。
//
// 两个键都**来回切**，不是单向进入：只能进不能出等于换个地方卡住。
//   Ctrl+' (FcitxKey_apostrophe) → 英文 ⇄ 拼音（B0。英文模式今天也进不去，
//                                   是「通路是否打通」的现成靶子）
//   Ctrl+\ (FcitxKey_backslash)  → 符号 ⇄ 拼音（B5）
// 不用 Ctrl+. ：那是 punctuation 模块的系统级占用（addon/punctuation.conf，
// Category=Module）；不用 ` ：拼音生态里那是 quickphrase。
// **不用 Ctrl+;（实测被剪贴板模块抢走）**：libclipboard.so 里就一行
// `Control+semicolon`，而该模块 Category=Module + OnDemand=False → 恒加载、
// 不分输入法。且它是 PreInputMethod 阶段的 watcher，排在输入法之前 ——
// 我们这边 filterAndAccept() 也拦不住它已经弹出来的面板（07-xx 的 harness 里
// e2e 阴性对照步实测到候选栏冒出剪贴板历史条目）。Ctrl+\ 是**逐个枚举**过
// 所有 fcitx5 库的 Control+/Super+/Alt+/Shift+ 组合后确认无占用的键。
//
// **为什么不改用可打印键**（2026-09-27 评估过，别再提，与 tsf-opi/src/vk.rs 同一条）：
// 裸键在中文模式下已被引擎的标点层认领 —— `\`→`、` 明写在 engine-core 的
// CHINESE_PUNCT，`` ` ``→`｀` 走 ascii_fullwidth 兜底，而中文模式**默认全角**；
// 裸 `'` 在缓冲为空时是引号。拿裸键当模式触发＝与用户裁决的标点功能抢同一个键。
//
// 不做键位可配置：一张两个条目的表，配置项等真有第二个人要改再说。
// 返回 true = 本键已被本插件消费。
static bool handleModeHotkey(fcitx::KeyEvent &keyEvent) {
    const fcitx::Key key = keyEvent.key();
    if (!key.states().test(fcitx::KeyState::Ctrl)) {
        return false;
    }
    const int32_t cur = opi_fcitx5_mode();
    int32_t target = cur;
    switch (static_cast<uint32_t>(key.sym())) {
    case FcitxKey_apostrophe:
        target = (cur == kModeEnglish) ? kModePinyin : kModeEnglish;
        break;
    case FcitxKey_backslash:
        target = (cur == kModeSymbol) ? kModePinyin : kModeSymbol;
        break;
    default:
        return false;
    }
    // 抬起一并消费、但不再切。只拦按下的话，客户端会收到**无 keydown 的 keyup**
    // （依赖键状态的游戏/编辑器卡键）—— 与 input_method.rs 里 last_printable
    // 「抬起按按下时记下的结论回复」是同一个理由。不返回来切也是同一个道理：
    // 按下切过去、抬起再切回来，用户看到的是模式纹丝不动。**重复**同理（见上）。
    if (!keyEvent.isRelease() && !isRepeatEvent(keyEvent)) {
        opi_fcitx5_switch_mode(target);
        // 切模式会动缓冲/候选（Rust 侧 switch_mode → engine.switch_mode），面板
        // 必须跟着走：漏了这步，切到英文后候选栏会继续挂着上一次的拼音候选。
        refreshingPanelFor(keyEvent.inputContext());
    }
    keyEvent.filterAndAccept();
    return true;
}

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

// fcitx::AddonInstance + fcitx::InputMethod 的组合基类在 fcitx5 5.1.x 里叫
// InputMethodEngine（fcitx/inputmethodengine.h:17，已继承 AddonInstance），自带
// `virtual void keyEvent(...) = 0` 与 `virtual void reset(...)`；仓库里没有别的
// `fcitx::InputMethod` 类型（grep Core/ 只有 InputMethodEngine V2/V3/V4），原先
// 多继承的两个基类都不存在。
class OpiEngine : public fcitx::InputMethodEngine {
public:
    OpiEngine(fcitx::Instance *instance) : instance_(instance) { loadDictionary(); }

    void keyEvent(const fcitx::InputMethodEntry &entry,
                  fcitx::KeyEvent &keyEvent) override;

    void reset(const fcitx::InputMethodEntry & /*entry*/,
               fcitx::InputContextEvent &event) override {
        // 同 keyEvent：这个入口抛出去也会带走整个守护进程。
        try {
            opi_fcitx5_clear();
            // Rust 侧清了缓冲，但面板是 fcitx5 侧的状态：不一并清，失焦/切换
            // 输入法后预编辑串与候选栏会继续挂在屏幕上（原码只清了引擎，没碰面板）。
            refreshingPanelFor(event.inputContext());
        } catch (const std::exception &e) {
            fprintf(stderr, "fcitx5-opi: reset 异常，已忽略: %s\n", e.what());
        }
    }

private:
    // keyEvent 的实现体。拆出来只为了在唯一的虚函数入口上套 try/catch ——
    // fcitx5 core **不捕获**引擎回调抛出的异常（实测：裸抛 → std::terminate →
    // 整个守护进程死掉）。Rust 侧每个导出入口都套了 catch_unwind，两侧对齐：
    // 把「整个输入法挂掉」降级成「这一次按键无效」。
    void keyEventImpl(const fcitx::InputMethodEntry &entry,
                      fcitx::KeyEvent &keyEvent);

    // XDG 数据目录下找 OPI 词库（B3：Rust 侧 opi_fcitx5_init_dict 把插件分发的
    // luna.opid 拷到 $XDG_DATA_HOME/opi/luna.opid，镜像 Android EngineLoader.
    // FILE_NAME；此处探测同一路径）。B3 接线在验收阶段完成。
    //
    // **必须接返回值**：Rust 侧 `install()` 是 `CandidateState::load(path)?` ——
    // 坏词库在 `?` 处提前返回，单例保持 None，`with_state` 恒为 None，所有导出
    // 函数退化成空操作。而 `install(坏路径) -> Err` 是 Rust 侧**有意**的语义
    // （lib.rs 的 install_singleton_fallback_and_path 断言「坏路径 → Err，不回
    // 退」），所以回退必须由本层做 —— 这正是原码丢返回值造成的洞：词库**存在但
    // 损坏**（下载不全/拷贝中断）时，插件进入「已加载、已注册、按键被接受、但一个
    // 字都不出，且任何地方都没有错误信息」的状态。实测（XDG_DATA_HOME 指向 6 字节
    // 垃圾 luna.opid）：四个键全 action=0；接上返回值后同场景回退内置词库，四键
    // action=1/1/1/2 并提交 '好'。
    //
    // 注意 `opi_fcitx5_load` 收的是**路径**不是词库内容（Rust 侧 read_utf8 →
    // install），故传 `path.data()` 是对的，不要改成传内容。
    void loadDictionary() {
        // StandardPath 自 5.1.21 起弃用，但**不能迁移**（老版本没有 StandardPaths）。
        // 为什么 / 边界在哪 / 何时删这三件事只写在一处：cpp/CMakeLists.txt 的
        // 「为什么两个 .cpp 里有 -Wdeprecated-declarations」段 —— 别在这里复制。
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wdeprecated-declarations"
        auto path = fcitx::StandardPath::global().locate(
            fcitx::StandardPath::Type::Data, "opi/luna.opid");
#pragma GCC diagnostic pop
        const bool ok =
            !path.empty() &&
            opi_fcitx5_load(reinterpret_cast<const uint8_t *>(path.data()), path.size());
        if (!ok) {
            if (!path.empty()) {
                // 「文件在、装不上」（下载不全/拷贝中断）。这行日志是必要的：否则
                // 词库损坏与「本来就没装词库」在用户侧无法区分，只表现为候选质量骤降。
                fprintf(stderr, "fcitx5-opi: 词库 %s 装载失败，回退内置词库\n", path.c_str());
            }
            opi_fcitx5_load(nullptr, 0); // 不存在或装载失败 → 内置回退词库
        }
    }

    fcitx::Instance *instance_;
};

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
    }
}

void OpiEngine::keyEventImpl(const fcitx::InputMethodEntry & /*entry*/,
                             fcitx::KeyEvent &keyEvent) {
    // 模式热键**先于** Rust 路由：Ctrl 组合键在 input_method.rs 首行就被直通，
    // 送进去也是白送（见 handleModeHotkey 上方说明）。
    if (handleModeHotkey(keyEvent)) {
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
    // 直通（action==0）：Rust 侧没消费这个键、状态没动，重推的是同一份内容 ——
    // 幂等，但白花一次跨 FFI + 候选 JSON 序列化/解析 + 客户端重绘，而 ESC /
    // 方向键 / F5 / PageDown 这类直通键是**每次按键**都来一遍。缓冲与上次推出
    // 去的一样就跳过（reset()/select() 不经过这里，各自已经推过）。
    if (result.action == 0 && take(opi_fcitx5_buffer()) == g_pushedBuffer) {
        return;
    }
    // 把引擎新状态推给前端：预编辑串（缓冲）+ 当前页候选。
    refreshingPanelFor(keyEvent.inputContext());
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
