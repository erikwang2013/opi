// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 动态插画皮肤（2026-10-31 用户裁决：动态背景就是皮肤，全端同一套）。
//
// 绘制在 `shared/scene`（Kotlin Compose），由 SceneArtGeneratorTest 渲成
// `shared/scene/art/scene-<id>.png`；构建前拷进 `resources/base/media/scene_<id>.png`。
// 本端只显示图片 + 用主题默认纯色皮肤取色。id 与其它端持久化字面值一致。

export class SceneSkin {
  id: string;
  label: string;
  /** `SKINS` 下标（主题自带的默认纯色皮肤）。 */
  skin: number;
  image: Resource;

  constructor(id: string, label: string, skin: number, image: Resource) {
    this.id = id;
    this.label = label;
    this.skin = skin;
    this.image = image;
  }
}

export const SCENES: SceneSkin[] = [
  new SceneSkin('meadow', '田园', 3, $r('app.media.scene_meadow')),
  new SceneSkin('forest', '森林', 11, $r('app.media.scene_forest')),
  new SceneSkin('garden', '花园', 5, $r('app.media.scene_garden')),
  new SceneSkin('night', '星夜', 9, $r('app.media.scene_night')),
  new SceneSkin('city', '城市', 6, $r('app.media.scene_city')),
  new SceneSkin('mountain', '山川', 6, $r('app.media.scene_mountain')),
  new SceneSkin('sea', '大海', 6, $r('app.media.scene_sea')),
  new SceneSkin('pond', '池塘', 5, $r('app.media.scene_pond')),
  new SceneSkin('autumn', '秋色', 8, $r('app.media.scene_autumn')),
  new SceneSkin('winter', '冬雪', 1, $r('app.media.scene_winter')),
  new SceneSkin('mist', '迷雾', 3, $r('app.media.scene_mist')),
  new SceneSkin('vegetable', '菜地', 5, $r('app.media.scene_vegetable')),
  new SceneSkin('sunset', '落日', 8, $r('app.media.scene_sunset')),
  new SceneSkin('study', '书房', 3, $r('app.media.scene_study')),
  new SceneSkin('kitchen', '灶台', 3, $r('app.media.scene_kitchen')),
  new SceneSkin('countryside', '乡村', 5, $r('app.media.scene_countryside')),
  new SceneSkin('streetlight', '路灯', 9, $r('app.media.scene_streetlight')),
  new SceneSkin('dogs', '狗群', 7, $r('app.media.scene_dogs')),
  new SceneSkin('cats', '猫群', 10, $r('app.media.scene_cats')),
];

export function sceneById(id: string): SceneSkin | undefined {
  return SCENES.find((s: SceneSkin) => s.id === id);
}

export function sceneLabel(id: string): string {
  const s: SceneSkin | undefined = sceneById(id);
  return s === undefined ? '' : s.label;
}

/** 背景图；未知 id 回 fallback（调用方只在 id 非空时调）。 */
export function sceneImage(id: string): Resource {
  const s: SceneSkin | undefined = sceneById(id);
  return s === undefined ? $r('app.media.scene_meadow') : s.image;
}
