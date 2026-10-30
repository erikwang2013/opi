// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 动态背景主题（2026-10-30 用户裁决：把当前这幅做成一套皮肤，再做几套不同风格
//! 的类似皮肤）。**同一套角色骨架，十九种场景/配色气质**：
//!
//! * [MEADOW] 田园：猫、兔、小人、气球、花草 —— 改动前的原画；
//! * [FOREST] 森林：松树、蘑菇、萤火虫，猫兔在林间；
//! * [GARDEN] 花园：向日葵、蜜蜂、浇水壶，小人当园丁；
//! * [NIGHT] 星夜：月亮、星星、灯笼、栅栏，猫和小人在夜色里；
//! * [CITY] 城市：楼群、路灯、小车；[MOUNTAIN] 山川：山脊、雪顶、松树；
//! * [SEA] 大海：海浪、帆船、灯塔；[POND] 池塘：荷叶、荷花、鸭子、蜻蜓；
//! * [AUTUMN] 秋色：橙冠树、南瓜、落叶；[WINTER] 冬雪：雪人、枯树、雪花；
//! * [MIST] 迷雾：雾带、雨伞、路灯；[VEGETABLE] 菜地：白菜、胡萝卜、稻草人；
//! * [SUNSET] 落日：低垂太阳、剪影山丘；[STUDY] 书房：书架/窗/书桌/台灯；
//! * [KITCHEN] 灶台：灶眼/锅/蒸汽/罐子；[COUNTRYSIDE] 乡村：农舍/风车/庄稼；
//! * [STREETLIGHT] 路灯：夜路/长椅/行人；[DOGS] 狗群：[CATS] 猫群：多只猫狗。
//!
//! id 是**持久化字面值**（桌面 `Background=` / Android 偏好 `kb_scene`）：
//! 改字面值 = 老配置静默回默认，与 `SkinOption.id` 同一条纪律。

package xyz.erik.opi.scene

enum class SceneTheme(val id: String, val label: String) {
    MEADOW("meadow", "田园"),
    FOREST("forest", "森林"),
    GARDEN("garden", "花园"),
    NIGHT("night", "星夜"),
    CITY("city", "城市"),
    MOUNTAIN("mountain", "山川"),
    SEA("sea", "大海"),
    POND("pond", "池塘"),
    AUTUMN("autumn", "秋色"),
    WINTER("winter", "冬雪"),
    MIST("mist", "迷雾"),
    VEGETABLE("vegetable", "菜地"),
    SUNSET("sunset", "落日"),
    STUDY("study", "书房"),
    KITCHEN("kitchen", "灶台"),
    COUNTRYSIDE("countryside", "乡村"),
    STREETLIGHT("streetlight", "路灯"),
    DOGS("dogs", "狗群"),
    CATS("cats", "猫群"),
    ;

    companion object {
        /** 未知/缺键 → 田园（改动前的行为），不抛。 */
        fun fromId(id: String?): SceneTheme =
            entries.firstOrNull { it.id == id } ?: MEADOW
    }
}
