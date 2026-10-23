// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 自定义短语的引擎薄壳（2026-10-23）：双 ABI 面共享这四个转发口。
//! 校验/匹配/导出格式全在 `engine_core::custom_phrases`，本层不加语义。

use super::Api;

impl Api {
    /// 导入 JSON。返回条数；失败（Err）不改动旧表。
    pub fn set_custom_phrases(&mut self, json: String) -> Result<usize, String> {
        self.router.engine_mut().set_custom_phrases(&json)
    }

    pub fn export_custom_phrases(&self) -> String {
        self.router.engine().export_custom_phrases()
    }

    pub fn set_custom_phrases_enabled(&mut self, on: bool) {
        self.router.engine_mut().set_custom_phrases_enabled(on);
    }

    pub fn custom_phrases_enabled(&self) -> bool {
        self.router.engine().custom_phrases_enabled()
    }
}
