// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 动态主题 ↔ 配色皮肤（2026-10-31 用户裁决：动态背景就是皮肤，不再单列一套
//! 「动态背景」选择）。每套主题带一款默认配色；选主题时同时把 `kb_skin` 写成
//! 这款，键面/候选栏/工具条都跟它走。
//!
//! 与桌面 `candidate/ThemeSkin.kt` 保持同一张表 —— 两端没有共享枚举，改表时两处
//! 一起改。

package xyz.erik.opi.keyboard

import xyz.erik.opi.scene.SceneTheme

/** 纯色皮肤（不含「不改动」）：清爽排首，作为默认皮肤。 */
internal val SOLID_SKINS: List<SkinOption> =
    listOf(SkinOption.FRESH) + SkinOption.entries.filter {
        it != SkinOption.FRESH && it != SkinOption.FOLLOW
    }

internal fun SceneTheme.defaultSkin(): SkinOption = when (this) {
    SceneTheme.MEADOW -> SkinOption.PAPER
    SceneTheme.FOREST -> SkinOption.PINE
    SceneTheme.GARDEN -> SkinOption.FRESH
    SceneTheme.NIGHT -> SkinOption.STARLIGHT
    SceneTheme.CITY -> SkinOption.AZURE
    SceneTheme.MOUNTAIN -> SkinOption.AZURE
    SceneTheme.SEA -> SkinOption.AZURE
    SceneTheme.POND -> SkinOption.FRESH
    SceneTheme.AUTUMN -> SkinOption.SUNSET
    SceneTheme.WINTER -> SkinOption.LIGHT
    SceneTheme.MIST -> SkinOption.PAPER
    SceneTheme.VEGETABLE -> SkinOption.FRESH
    SceneTheme.SUNSET -> SkinOption.SUNSET
    SceneTheme.STUDY -> SkinOption.PAPER
    SceneTheme.KITCHEN -> SkinOption.PAPER
    SceneTheme.COUNTRYSIDE -> SkinOption.FRESH
    SceneTheme.STREETLIGHT -> SkinOption.STARLIGHT
    SceneTheme.DOGS -> SkinOption.LEMON
    SceneTheme.CATS -> SkinOption.WISTERIA
}
