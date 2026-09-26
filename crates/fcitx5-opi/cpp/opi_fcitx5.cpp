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

#include <cstdint>
#include <cstdio>
#include <memory>
#include <string>
#include <utility>
#include <vector>

// ---------- Rust C 出口声明（与 src/lib.rs 的 #[repr(C)]/no_mangle 对应） ----------

// Rust 侧 OpString { ptr, len }（UTF-8，非 NUL 结尾；ptr==nullptr 视为空串）。
struct OpiString {
    const uint8_t *ptr;
    size_t len;
};

// Rust 侧 KeyEventResult { action, text }：action 0=直通 1=已处理 2=提交。
struct OpiKeyEventResult {
    int32_t action;
    OpiString text;
};

extern "C" {
bool opi_fcitx5_load(const uint8_t *ptr, size_t len);
OpiString opi_fcitx5_input_key(const uint8_t *ptr, size_t len);
void opi_fcitx5_backspace();
void opi_fcitx5_clear();
OpiString opi_fcitx5_select(size_t index);
void opi_fcitx5_switch_mode(int32_t mode); // 0=Pinyin 1=English 2=Number 3=Symbol
void opi_fcitx5_set_shift(bool on);
OpiString opi_fcitx5_input_space();
OpiString opi_fcitx5_candidates(size_t limit); // JSON 数组（UTF-8）
OpiString opi_fcitx5_buffer();
int32_t opi_fcitx5_mode();
OpiKeyEventResult opi_fcitx5_key_event(uint32_t keyval, uint32_t states);
void opi_ffi_free_string_utf8(OpiString s);
}

// 取走并释放 Rust 侧字符串。
static std::string take(OpiString s) {
    // ptr==nullptr 视为空串（Rust 侧 OpString::empty 的表示）；仅守卫构造，
    // free 契约不变：每个返回的 OpString 恰好 free 一次。
    std::string out;
    if (s.ptr != nullptr) {
        out.assign(reinterpret_cast<const char *>(s.ptr), s.len);
    }
    opi_ffi_free_string_utf8(s);
    return out;
}

// ---------- 输入面板：预编辑 + 候选栏 ----------
//
// ⚠️ UI 通路**不在** AddonInstance 的虚表里。setPreedit / setCandidateList 是
// InputPanel 的方法（fcitx/inputpanel.h:55 / :68），updateUserInterface 是
// InputContext 的方法（fcitx/inputcontext.h:210）。引擎要**主动调用**它们，
// 没有「少 override 了哪个虚函数」这回事 —— 原码的问题是这些函数一次都没被
// 调用过（`InputMethodEngine` 里确实没有对应的虚函数可 override）。
//
// 本文件只推**当前页**给前端：Rust 侧 opi_fcitx5_candidates 返回的已经是
// 切片后的当前页（candidate.rs 的 candidates() 按 page*PAGE_SIZE 切片），
// 翻页由 Rust 路由驱动（input_method.rs 的 PageUp/PageDown → prev/next_page）。
// 若把整份候选交给 CommonCandidateList 自己去翻，C++ 与 Rust 就各存一份页码，
// 前端点翻页箭头时两边立刻漂移。
//
// 取页大小镜像 Rust 侧 candidate.rs 的 PAGE_SIZE；Rust 若改这个数，这里跟着改
// （取回条数少于它也无副作用，只是不再满页）。
static constexpr size_t kOpiPageSize = 8;

// 解析 opi_fcitx5_candidates 的返回值：serde_json 对 Vec<String> 的输出
// （`["好","你"]` —— 非 ASCII 直出 UTF-8，不转义成 \uXXXX，除非原字符是控制
// 字符）。**只认这一个形状**，不做通用 JSON：这里只有一处调用，通用解析器的
// 复杂度在这个文件里换不回任何东西。
static bool parseHex4(const std::string &s, size_t pos, uint32_t &out) {
    if (pos + 4 > s.size()) {
        return false;
    }
    uint32_t v = 0;
    for (size_t i = 0; i < 4; ++i) {
        const char c = s[pos + i];
        v <<= 4;
        if (c >= '0' && c <= '9') {
            v |= static_cast<uint32_t>(c - '0');
        } else if (c >= 'a' && c <= 'f') {
            v |= static_cast<uint32_t>(c - 'a' + 10);
        } else if (c >= 'A' && c <= 'F') {
            v |= static_cast<uint32_t>(c - 'A' + 10);
        } else {
            return false;
        }
    }
    out = v;
    return true;
}

static void appendUtf8(std::string &out, uint32_t cp) {
    if (cp < 0x80) {
        out.push_back(static_cast<char>(cp));
    } else if (cp < 0x800) {
        out.push_back(static_cast<char>(0xC0 | (cp >> 6)));
        out.push_back(static_cast<char>(0x80 | (cp & 0x3F)));
    } else if (cp < 0x10000) {
        out.push_back(static_cast<char>(0xE0 | (cp >> 12)));
        out.push_back(static_cast<char>(0x80 | ((cp >> 6) & 0x3F)));
        out.push_back(static_cast<char>(0x80 | (cp & 0x3F)));
    } else {
        out.push_back(static_cast<char>(0xF0 | (cp >> 18)));
        out.push_back(static_cast<char>(0x80 | ((cp >> 12) & 0x3F)));
        out.push_back(static_cast<char>(0x80 | ((cp >> 6) & 0x3F)));
        out.push_back(static_cast<char>(0x80 | (cp & 0x3F)));
    }
}

static std::vector<std::string> parseJsonStringArray(const std::string &json) {
    std::vector<std::string> out;
    size_t i = 0;
    auto skipWs = [&json, &i] {
        while (i < json.size() && (json[i] == ' ' || json[i] == '\t' ||
                                   json[i] == '\n' || json[i] == '\r')) {
            ++i;
        }
    };
    skipWs();
    if (i >= json.size() || json[i] != '[') {
        return out; // 空串（Rust 侧未装载/出错时返回值）也走这里
    }
    ++i;
    while (true) {
        skipWs();
        if (i >= json.size() || json[i] != '"') {
            break;
        }
        ++i;
        std::string s;
        bool closed = false;
        while (i < json.size()) {
            const char c = json[i++];
            if (c == '"') {
                closed = true;
                break;
            }
            if (c != '\\') {
                s.push_back(c);
                continue;
            }
            if (i >= json.size()) {
                break;
            }
            const char esc = json[i++];
            if (esc == 'u') {
                uint32_t cp = 0;
                if (!parseHex4(json, i, cp)) {
                    i = json.size(); // 坏转义：收摊，已解析的部分照常返回
                    break;
                }
                i += 4;
                // 代理对（😀）：低半区必须紧跟，否则按单码点编码。
                if (cp >= 0xD800 && cp <= 0xDBFF && i + 6 <= json.size() &&
                    json[i] == '\\' && json[i + 1] == 'u') {
                    uint32_t lo = 0;
                    if (parseHex4(json, i + 2, lo) && lo >= 0xDC00 &&
                        lo <= 0xDFFF) {
                        cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                        i += 6;
                    }
                }
                appendUtf8(s, cp);
                continue;
            }
            switch (esc) {
            case 'b':
                s.push_back('\b');
                break;
            case 'f':
                s.push_back('\f');
                break;
            case 'n':
                s.push_back('\n');
                break;
            case 'r':
                s.push_back('\r');
                break;
            case 't':
                s.push_back('\t');
                break;
            default:
                s.push_back(esc); // \" \\ \/ 及未知转义：取字面字符
                break;
            }
        }
        if (!closed) {
            break;
        }
        out.push_back(std::move(s));
        skipWs();
        if (i < json.size() && json[i] == ',') {
            ++i;
            continue;
        }
        break;
    }
    return out;
}

// 候选词：点选后走 Rust 侧 select(**页内**索引) 并提交。
// CandidateWord::select 是纯虚（fcitx/candidatelist.h:44），必须实现。
class OpiCandidateWord : public fcitx::CandidateWord {
public:
    OpiCandidateWord(std::string text, size_t index)
        : fcitx::CandidateWord(fcitx::Text(std::move(text))), index_(index) {}
    void select(fcitx::InputContext *ic) const override;

private:
    // 页内索引（与 Rust 侧 CandidateState::select 同语义，见 candidate.rs
    // `page * PAGE_SIZE + index`）。
    size_t index_;
};

// 把 Rust 侧当前状态（缓冲 = 预编辑串，当前页候选）推到 fcitx5 面板。
static void refreshingPanelFor(fcitx::InputContext *ic);

void OpiCandidateWord::select(fcitx::InputContext *ic) const {
    const std::string out = take(opi_fcitx5_select(index_));
    if (ic != nullptr && !out.empty()) {
        ic->commitString(out);
    }
    // 选完 Rust 侧缓冲/候选都变了，面板必须同步 —— 否则候选栏会留在屏幕上。
    refreshingPanelFor(ic);
}

static void refreshingPanelFor(fcitx::InputContext *ic) {
    if (ic == nullptr) {
        return;
    }
    auto &panel = ic->inputPanel();
    const std::string buffer = take(opi_fcitx5_buffer());
    if (buffer.empty()) {
        // 缓冲空 = 本引擎当前无 composing。用 reset() 而不是 setPreedit("")：
        // 它把预编辑、候选表、辅助串一起清干净，正是「无 composing」的完整表达。
        panel.reset();
    } else {
        fcitx::Text preedit(buffer);
        // 光标在预编辑串末尾（按字节；拼音缓冲是 ASCII，字节序即字符序）。
        preedit.setCursor(static_cast<int>(buffer.size()));
        panel.setPreedit(preedit);

        auto list = std::make_unique<fcitx::CommonCandidateList>();
        // 页大小 = 推过去的条数上限，故 C++ 侧候选表恒为单页：翻页只由 Rust
        // 驱动，前端不会画出与 Rust 页码不一致的翻页箭头（见上方说明）。
        list->setPageSize(static_cast<int>(kOpiPageSize));
        const auto texts =
            parseJsonStringArray(take(opi_fcitx5_candidates(kOpiPageSize)));
        for (size_t i = 0; i < texts.size(); ++i) {
            list->append<OpiCandidateWord>(texts[i], i);
        }
        panel.setCandidateList(std::move(list));
    }
    // 通知前端刷新。immediate=false（默认）：fcitx5 会把同一轮事件里的多次
    // 更新合并成一次 UI 刷新，这是引擎该走的路。
    ic->updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
}

// ---------- fcitx5 插件本体（结构对齐 fcitx5 example/ime.cpp） ----------

// fcitx::AddonInstance + fcitx::InputMethod 的组合基类在 fcitx5 5.1.x 里叫
// InputMethodEngine：fcitx/inputmethodengine.h:17
//   class FCITXCORE_EXPORT InputMethodEngine : public AddonInstance
// 并自带 `virtual void keyEvent(...) = 0` 与 `virtual void reset(...)`。
// 仓库里没有别的 `fcitx::InputMethod` 类型（grep Core/ 只有 InputMethodEngine、
// InputMethodEngineV2/V3/V4），原先的多继承两个基类都不存在。
class OpiEngine : public fcitx::InputMethodEngine {
public:
    OpiEngine(fcitx::Instance *instance) : instance_(instance) { loadDictionary(); }

    void keyEvent(const fcitx::InputMethodEntry &entry,
                  fcitx::KeyEvent &keyEvent) override;

    void reset(const fcitx::InputMethodEntry & /*entry*/,
               fcitx::InputContextEvent &event) override {
        opi_fcitx5_clear();
        // Rust 侧清了缓冲，但面板是 fcitx5 侧的状态：不一并清，失焦/切换输入法
        // 后预编辑串与候选栏会继续挂在屏幕上（原码只清了引擎，没碰面板）。
        refreshingPanelFor(event.inputContext());
    }

private:
    // XDG 数据目录下找 OPI 词库（B3：Rust 侧 opi_fcitx5_init_dict 把插件分发
    // 的 luna.opid 拷到 $XDG_DATA_HOME/opi/luna.opid——文件名镜像 Android
    // EngineLoader.FILE_NAME；此处探测同一路径）。B3 接线在验收阶段完成。
    // 找不到时 load(nullptr, 0) → Rust 侧使用内置回退词库。
    //
    // **必须接返回值**：`opi_fcitx5_load` 返回装没装上。Rust 侧 `install()` 是
    // `CandidateState::load(path)?` —— 坏词库在 `?` 处提前返回，单例保持 None，
    // 于是 `with_state` 恒为 None，所有导出函数退化成空操作。
    // 而 `install(坏路径) -> Err` 是 Rust 侧**有意**的语义（lib.rs 的
    // install_singleton_fallback_and_path 测试就断言了「坏路径 → Err，不回退」），
    // 所以「回退」这件事必须由本层来做 —— 这正是原码丢返回值造成的洞：
    // 词库**存在但损坏**（下载不全/拷贝中断）时，插件进入「已加载、已注册、
    // 按键被接受、但一个字都不出，且任何地方都没有错误信息」的状态。
    // 实测（XDG_DATA_HOME 指向 6 字节垃圾 luna.opid）：四个键全 action=0；
    // 接上返回值后同样场景回退内置词库，四键 action=1/1/1/2 并提交 '好'。
    //
    // 注意 `opi_fcitx5_load` 收的是**路径**不是词库内容（Rust 侧 read_utf8 →
    // install），故传 `path.data()` 是对的，不要改成传内容。
    void loadDictionary() {
        auto path = fcitx::StandardPath::global().locate(
            fcitx::StandardPath::Type::Data, "opi/luna.opid");
        const bool ok =
            !path.empty() &&
            opi_fcitx5_load(reinterpret_cast<const uint8_t *>(path.data()), path.size());
        if (!ok) {
            if (!path.empty()) {
                // 能走到这里就是「文件在、装不上」（下载不全/拷贝中断）。这行日志
                // 是必要的：否则词库损坏与「本来就没装词库」在用户侧完全无法区分，
                // 只会表现为候选质量骤降。
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

void OpiEngine::keyEvent(const fcitx::InputMethodEntry & /*entry*/,
                         fcitx::KeyEvent &keyEvent) {
    // keyval 取 xkb keysym（ASCII 段与 Unicode 码点一致）；states 用
    // KeyEventBase::key() 的 KeyStates，再补全 Rust 侧线格式的三位。
    //
    // 取 key()（normalize 后的 Key）而非 rawKey()：key() 已把 states 收敛到
    // ctrl/alt/shift/super（fcitx-utils/key.h:157 `Key normalize() const;`），
    // 值域小且与前端无关。
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
    // VirtualKeyboardEvent::isLongPress()（fcitx/event.h:387，触屏虚拟键盘专用），
    // 普通物理键事件拿不到。此处不合成该位 —— 恒为 0，Rust 侧
    // handle_shift 的长按分支（→ shift_long_press）在 fcitx5 桌面端不可达，
    // 需 B3 另行决定入口（CapsLock 或按住超时）。见 README.md「已知边界」。
    const OpiKeyEventResult result = opi_fcitx5_key_event(keyEvent.key().sym(), states);
    switch (result.action) {
    case 2: // Commit：提交文本并消费按键
        // commitString 是 InputContext 的成员（fcitx/inputcontext.h:177），
        // 不是引擎的 —— 原写法把 inputContext() 当参数传给一个不存在的成员。
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
    // 把引擎新状态推给前端：预编辑串（缓冲）+ 当前页候选。直通（action==0）的
    // 按键不改缓冲，这里重推的是同一份内容，幂等。
    refreshingPanelFor(keyEvent.inputContext());
}

// 工厂类：FCITX_ADDON_FACTORY 宏（fcitx/addoninstance.h:193）只做
//   static ClassName factory; return &factory;
// 即它要求 ClassName 这个类型已经存在，自己并不定义。上一版只写了宏调用，
// 仓库里从没有 OpiEngineFactory 的定义，因此这一行是编译错误的另一来源。
class OpiEngineFactory : public fcitx::AddonFactory {
public:
    fcitx::AddonInstance *create(fcitx::AddonManager *manager) override {
        return new OpiEngine(manager->instance());
    }
};

FCITX_ADDON_FACTORY(OpiEngineFactory);
