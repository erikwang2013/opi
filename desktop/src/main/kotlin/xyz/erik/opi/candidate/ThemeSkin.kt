// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 动态主题 ↔ 配色皮肤（2026-10-31 用户裁决：动态背景就是皮肤，不再单列一套
//! 「动态背景」选择）。每套主题带一款**默认配色**；选主题时把 `Skin=` 一并写成
//! 这款，fcitx5 全局主题、候选窗、状态栏、设置预览都跟它走。
//!
//! 映射只决定配色，不改变主题的插画/道具；同一个配色可服务多套主题（如 AZURE
//! 给城市/山川/大海）。Android 的 `keyboard/ThemeSkin.kt` 保持同一张表 —— 两端
//! 没有共享枚举，改表时两处一起改。

package xyz.erik.opi.candidate

import xyz.erik.opi.scene.SceneTheme

/** 纯色皮肤（不含「不改动」）：清爽排首，作为默认皮肤。 */
internal val SOLID_SKINS: List<SkinOption> =
    listOf(SkinOption.FRESH) + SkinOption.entries.filter {
        it != SkinOption.FRESH && it != SkinOption.KEEP
    }

/** 主题自带的配色皮肤。 */
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

/**
 * 状态栏 logo 单击的下一款皮肤：先按 [SOLID_SKINS] 走纯色，再按枚举序走动态主题，
 * 最后绕回清爽。返回 `background == null` 表示纯色。
 */
internal fun nextSkinChoice(
    background: SceneTheme?,
    skin: SkinOption,
): Pair<SceneTheme?, SkinOption> {
    if (background == null) {
        val i = SOLID_SKINS.indexOf(skin)
        if (i >= 0 && i + 1 < SOLID_SKINS.size) return null to SOLID_SKINS[i + 1]
        val first = SceneTheme.entries.first()
        return first to first.defaultSkin()
    }
    val i = SceneTheme.entries.indexOf(background)
    if (i + 1 < SceneTheme.entries.size) {
        val next = SceneTheme.entries[i + 1]
        return next to next.defaultSkin()
    }
    return null to SOLID_SKINS.first()
}
