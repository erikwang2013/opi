// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `ITfInputProcessorProfileActivationSink`：输入法**配置切换**时释放候选管道
//! （Windows 目标专属，从 `tsf.rs` 拆出守 500 行）。
//!
//! 为什么 `Deactivate` 不够：TSF 的 `ITfTextInputProcessor::Deactivate` 通常在
//! 切到别的输入法时会被调用，但真机报障（2026-10-22「切换到其他输入法状态栏
//! 依然存在」）说明只靠它仍会留下仍持管道的宿主。`OnActivated` 是 TSF 对
//! **配置切换本身**的通知：另一个输入法激活 → 立即断开，状态栏下一次 1s 轮询
//! 读不到 `im=opi` 就会隐藏。
//!
//! 只处理 `TF_PROFILETYPE_INPUTPROCESSOR`：键盘布局切换（同语言内换 HKL）时本
//! TIP 可能仍有效，不能因此把中文候选断开。本 profile 的 CLSID/GUID 与注册处
//! 同源（`dll.rs` 的字面量），不手抄第二份。

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use windows::Win32::UI::Input::KeyboardAndMouse::HKL;
use windows::Win32::UI::TextServices::{
    ITfInputProcessorProfileActivationSink, ITfInputProcessorProfileActivationSink_Impl,
    TF_IPSINK_FLAG_ACTIVE, TF_PROFILETYPE_INPUTPROCESSOR,
};
use windows::core::{GUID, Result, implement};

use crate::com_server::parse_guid;
use crate::dll::{CLSID_TEXT_SERVICE, GUID_PROFILE};
use crate::sink::{TsfSink, ensure_panic_hook};

#[implement(ITfInputProcessorProfileActivationSink)]
pub(crate) struct ProfileSink {
    sink: Arc<dyn TsfSink>,
    clsid: GUID,
    profile: GUID,
}

impl ProfileSink {
    pub(crate) fn new(sink: Arc<dyn TsfSink>) -> Result<Self> {
        Ok(Self {
            sink,
            clsid: parse_guid(CLSID_TEXT_SERVICE)?,
            profile: parse_guid(GUID_PROFILE)?,
        })
    }
}

impl ITfInputProcessorProfileActivationSink_Impl for ProfileSink_Impl {
    fn OnActivated(
        &self,
        profile_type: u32,
        _langid: u16,
        clsid: *const GUID,
        _catid: *const GUID,
        profile: *const GUID,
        _hkl: HKL,
        flags: u32,
    ) -> Result<()> {
        ensure_panic_hook();
        // 键盘布局切换（同语言内换 HKL）不代表本 TIP 失效；那一档不碰候选管道，
        // 交给 Deactivate / 线程焦点。只处理输入法配置本身。
        if profile_type != TF_PROFILETYPE_INPUTPROCESSOR {
            return Ok(());
        }
        // 两个 GUID 指针按 TSF 约定可能为 NULL；不判空就解引用是宿主进程里的
        // 访问冲突。
        let ours = flags & TF_IPSINK_FLAG_ACTIVE != 0
            && !clsid.is_null()
            && !profile.is_null()
            // SAFETY: 非空且由 TSF 保证指向本次调用有效的 GUID。
            && unsafe { *clsid == self.clsid && *profile == self.profile };
        // 我们的 profile 激活 → 连接；其余输入法激活 / 我们被取消激活 → 断开。
        // 断开是幂等的；候选窗下一次 show 会惰性重连（仅在我们再次激活后）。
        let sink = Arc::clone(&self.sink);
        catch_unwind(AssertUnwindSafe(move || {
            if ours {
                sink.on_activate();
            } else {
                sink.disconnect();
            }
        }))
        .ok();
        Ok(())
    }
}
