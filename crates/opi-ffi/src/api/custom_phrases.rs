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

    // ---- 英文套（2026-10-24）：与中文套同 schema/同校验，表与开关完全独立。 ----

    pub fn set_english_custom_phrases(&mut self, json: String) -> Result<usize, String> {
        self.router.engine_mut().set_english_custom_phrases(&json)
    }

    pub fn export_english_custom_phrases(&self) -> String {
        self.router.engine().export_english_custom_phrases()
    }

    pub fn set_english_custom_phrases_enabled(&mut self, on: bool) {
        self.router
            .engine_mut()
            .set_english_custom_phrases_enabled(on);
    }

    pub fn english_custom_phrases_enabled(&self) -> bool {
        self.router.engine().english_custom_phrases_enabled()
    }

    /// 英文短语是否实际可命中（开关开 + 表非空）：Android 普通英文据此改走缓冲。
    pub fn english_custom_phrases_active(&self) -> bool {
        self.router.engine().english_custom_phrases_active()
    }

    /// install 前快照两套短语（`install` 整体换引擎，必须回放）。
    pub fn export_custom_phrase_sets(&self) -> (String, bool, String, bool) {
        (
            self.export_custom_phrases(),
            self.custom_phrases_enabled(),
            self.export_english_custom_phrases(),
            self.english_custom_phrases_enabled(),
        )
    }

    /// install 后回放两套短语；坏 JSON 整份留在引擎旧表语义（引擎侧原子）。
    pub fn restore_custom_phrase_sets(&mut self, saved: (String, bool, String, bool)) {
        let (json, on, json_en, on_en) = saved;
        let _ = self.set_custom_phrases(json);
        self.set_custom_phrases_enabled(on);
        let _ = self.set_english_custom_phrases(json_en);
        self.set_english_custom_phrases_enabled(on_en);
    }
}
