// OPI fcitx5 插件胶水（B2 计划缺口）：Rust 逻辑出口（libfcitx5_opi.so 的
// C 符号）与 fcitx5 5.1.x AddonInstance/InputMethod 之间的薄层。
//
// 行为路由全部在 Rust 侧（crates/fcitx5-opi/src/input_method.rs，镜像
// Android KeyRouter），本文件只做：键事件 → opi_fcitx5_key_event →
// 按动作码（0=直通 1=已处理 2=提交）行事。字符串约定 UTF-8+长度，
// 返回值由 Rust 侧分配，用 opi_ffi_free_string_utf8 释放。
//
// 编译期检查：本机无 fcitx5-dev 头（pkg-config 无 fcitx5，
// /usr/include/fcitx5 不存在），未在本机编译；见 README.md。

#include <fcitx/addonfactory.h>
#include <fcitx/addoninstance.h>
#include <fcitx/inputcontext.h>
#include <fcitx/inputmethodengine.h>
#include <fcitx/instance.h>
#include <fcitx/keyevent.h>
#include <fcitx-utils/standardpath.h>

#include <cstdint>
#include <string>

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

// ---------- fcitx5 插件本体（结构对齐 fcitx5 example/ime.cpp） ----------

class OpiEngine : public fcitx::AddonInstance, public fcitx::InputMethod {
public:
    OpiEngine(fcitx::Instance *instance) : instance_(instance) { loadDictionary(); }

    void keyEvent(const fcitx::InputMethodEntry &entry,
                  fcitx::KeyEvent &keyEvent) override;

    void reset(const fcitx::InputMethodEntry &entry,
               fcitx::InputContextEvent &event) override {
        opi_fcitx5_clear();
    }

private:
    // XDG 数据目录下找 OPI 词库；B3 将接线真实文件，现只做简单探测。
    // 找不到时 load(nullptr, 0) → Rust 侧使用内置回退词库。
    void loadDictionary() {
        auto path = fcitx::StandardPath::global().locate(
            fcitx::StandardPath::Type::Data, "opi/opi.dict");
        if (path.empty()) {
            opi_fcitx5_load(nullptr, 0);
        } else {
            opi_fcitx5_load(reinterpret_cast<const uint8_t *>(path.data()), path.size());
        }
    }

    fcitx::Instance *instance_;
};

void OpiEngine::keyEvent(const fcitx::InputMethodEntry &entry,
                         fcitx::KeyEvent &keyEvent) {
    // keyval 取 xkb keysym（ASCII 段与 Unicode 码点一致）；states 补全
    // fcitx5 KeyState 位（Released=1<<26 Repeat=1<<27 LongPressed=1<<28），
    // 与 Rust 侧 input_method 常量一一对应。
    uint32_t states = static_cast<uint32_t>(keyEvent.states());
    if (keyEvent.isRelease()) {
        states |= 1u << 26;
    }
    if (keyEvent.isRepeat()) {
        states |= 1u << 27;
    }
    if (keyEvent.isLongPressed()) { // fcitx5 ≥ 5.1
        states |= 1u << 28;
    }
    const OpiKeyEventResult result = opi_fcitx5_key_event(keyEvent.key().sym(), states);
    switch (result.action) {
    case 2: // Commit：提交文本并消费按键
        commitString(keyEvent.inputContext(), take(result.text));
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
}

FCITX_ADDON_FACTORY(OpiEngineFactory);
