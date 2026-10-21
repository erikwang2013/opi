// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `ITfThreadFocusSink`：线程焦点 → 候选管道持有者（Windows 目标专属，从 `tsf.rs` 拆出守 500 行）。
//!
//! 为什么必须有这一层：TSF 的 `ITfTextInputProcessor::Deactivate` **不随应用切换触发**
//! （它只在输入法被取消激活时触发）。只靠 Activate/Deactivate 时，先激活过 opi 的后台
//! 宿主会一直占着单实例候选管道：状态栏读到的是后台宿主的 `im=opi`（切走不消失），前台
//! 应用连不上候选窗（中文模式无候选），状态栏动作也打到错误的宿主上。
//!
//! 焦点进入本线程 → `on_activate`（主动连接）；焦点离开 → `disconnect`（断开、交还管道）。
//! 两个回调都必须兜住 panic：它们经 COM vtable 回到宿主进程。

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use windows::Win32::UI::TextServices::{ITfThreadFocusSink, ITfThreadFocusSink_Impl};
use windows::core::{Result, implement};

use crate::sink::{TsfSink, ensure_panic_hook};

#[implement(ITfThreadFocusSink)]
pub(crate) struct ThreadFocusSink {
    sink: Arc<dyn TsfSink>,
}

impl ThreadFocusSink {
    pub(crate) fn new(sink: Arc<dyn TsfSink>) -> Self {
        Self { sink }
    }
}

impl ITfThreadFocusSink_Impl for ThreadFocusSink_Impl {
    fn OnSetThreadFocus(&self) -> Result<()> {
        ensure_panic_hook();
        let sink = Arc::clone(&self.sink);
        catch_unwind(AssertUnwindSafe(|| sink.on_activate())).ok();
        Ok(())
    }

    fn OnKillThreadFocus(&self) -> Result<()> {
        ensure_panic_hook();
        let sink = Arc::clone(&self.sink);
        catch_unwind(AssertUnwindSafe(|| sink.disconnect())).ok();
        Ok(())
    }
}
