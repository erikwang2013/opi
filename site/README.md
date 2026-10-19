# opi 静态站点

项目介绍 / 功能迭代 / 版本日志 / 应用下载的静态单页。无前端框架，翻屏使用原生
CSS scroll-snap；首屏的 three.js 场景只画小欧与粒子环，离屏暂停。

## 本地预览

```bash
# 在仓库根目录执行；站点根目录就是 site/
python3 -m http.server 8080 --directory site
# 打开 http://localhost:8080/
```

直接双击 `index.html` 也能看，但 `//busuanzi...` 的统计脚本需要 HTTP(S) 才会工作。

## 结构

```
site/
  index.html            语义化单页（七个翻屏 section）
  css/base.css          设计 token、壳层、按钮、页脚
  css/sections.css      hero / 小欧 / 十二形态 / 能力 / 迭代 / 版本 / 下载
  css/responsive.css    1120 / 900 / 720 / 460 四档断点
  js/site.js            导航高亮、进度条、reveal、形态弹窗
  js/pet3d.js           three.js：宠物纹理 + 光环 + 粒子 + 指针视差
  vendor/               three.js 0.170.0（module 版，随站点分发）
  assets/pet/           小欧 PNG / SVG、十二形态 GIF+PNG
  assets/social-preview.png  Open Graph 预览图（1280×640，小欧主题）
```

## 性能预算

- 无 React/Vue、无外部字体、无阻塞脚本；CSS 与 JS 都是本地小文件。
- 十二张 GIF 全部 `loading="lazy"`；首屏只加载小欧 PNG 与 three.js。
- three.js 渲染器 `powerPreference: low-power`、关闭抗锯齿、DPR 上限 1.7（移动端 1.4）；
  hero 离开视口即停止渲染，页面隐藏即停。
- 系统开启「减少动态效果」时：不跑动画循环，形态显示 PNG 静帧。
- 交互（导航 / 弹窗 / 滚动高亮）只做一次 rAF 批处理，不读布局之外的属性；
  目标是把单次交互控制在 50ms 内。本地无头 Chrome 实测（1440×900）：
  导航点击 → 下一帧约 40ms，形态弹窗约 36ms（含等待两帧 rAF）；这不是发布门禁，
  实际值受设备与浏览器影响，发布前用 Lighthouse / DevTools Performance 复测。

## 部署

把 `site/` 整个目录作为静态站点根目录即可（Nginx / Caddy / Cloudflare Pages /
GitHub Pages 均可）。页面内没有构建步骤，也没有服务端依赖。

- `site/README.md` 的链接若部署在子路径（如 `/opi/`），保持相对路径即可。
- 统计脚本由 busuanzi 提供；广告拦截器或离线环境会把它拦掉，页面其余部分不受影响。
- 版本号在 `index.html`（hero 的 `v1.16.1`）与发布页链接里是手写的，发版时同步。

## 素材授权

小欧与十二形态来自 `shared/pet/`，授权见仓库根目录
[`ASSETS-LICENSE.md`](../ASSETS-LICENSE.md)；three.js 为 MIT，未改动其版权头。
