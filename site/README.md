# opi 静态站点

项目介绍 / 功能迭代 / 版本日志 / 应用下载的静态站点，中文 `/` 与英文 `/en/` 双页面。无前端框架，翻屏使用原生
CSS scroll-snap；首屏的 three.js 场景只画小欧与粒子环，离屏暂停；其余分节共用一层
纯 CSS 3D 键帽/光环环境，首屏加载时显示 3D 小欧加载模块。

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
  index.html            中文首页（七个 section：认识/能力/迭代/形态/版本/下载；SEO 元数据见下）
  en/index.html         英文首页（同结构；EN / 中文切换，独立 canonical 与 hreflang）
  version.json          机器可读的最新版本（客户端「检测更新」首选；发版时与版本号一起改）
  robots.txt            抓取规则（屏蔽仓库说明 Markdown）与 sitemap 声明
  download/index.html   /download 入口，转场到首页下载区（noindex）
  sitemap.xml           双语站点地图（zh / en，含 hreflang alternates）
  css/base.css          设计 token、壳层、按钮、页脚
  css/sections.css      hero / 小欧 / 十二形态 / 能力 / 迭代 / 版本 / 下载
  css/components.css    版本板 / 下载卡 / 页脚 / 弹层 / 回忆时光机
  css/responsive.css    1120 / 900 / 720 / 460 四档断点
  js/site.js            导航高亮、进度条、reveal、形态弹窗、打赏二维码弹框
  js/pet3d.js           three.js：宠物纹理 + 光环 + 粒子 + 指针视差
  vendor/               three.js 0.170.0（module 版，随站点分发）
  assets/pet/           小欧 PNG / SVG、十二形态 WebP+PNG（GIF 原图保留）
  assets/donate/        微信 / 支付宝收款二维码（捐赠卡片）
  assets/social-preview.png  Open Graph 预览图（1280×640，小欧主题）
```

## 性能预算

- 无 React/Vue、无外部字体、无阻塞脚本；CSS 与 JS 都是本地小文件。
- 十二张形态图先加载 PNG 静帧（约 130 KB），进入形态区前 300px 才切换为动图 WebP
  （约 311 KB）；离开后切回静帧。系统开启「减少动态效果」时始终保持 PNG，
  不支持动图 WebP 的浏览器自动回退到原 GIF。
- three.js 延迟到首屏空闲后再加载，且只在桌面、硬件 WebGL 可用且未开启 Save-Data 时加载；
  手机与软件渲染（SwiftShader/llvmpipe）使用轻量 CSS 宠物。DPR 上限 1.25，首屏 3D 以 30 fps 运行。
- three.js 在 hero 离屏、页面隐藏或「时光机」打开时完全停止帧循环，关闭后恢复。
- 全站 3D 环境层与加载模块都走 CSS 3D（`transform` + 合成器），不额外创建 WebGL
  上下文，也不占主线程帧循环；手机隐藏环境层以降功耗。打赏二维码用原生 `showModal()`
  原地放大，Esc / 点遮罩 / 关闭键都可退出。
- 版本列表用 `content-visibility` 延后离屏布局；进入附近时**默认停在最新版本（顶部）**，
  不再自动跳到 v1.0.0；初始加载不为 76 条历史版本付布局成本。
- 「回忆时光机」不再首访自动弹出，只由「回忆」按钮打开；打开仍走两帧外壳+内容策略。
- 形态弹窗使用 `dialog.show()` + 自定义遮罩与焦点循环，避免 `showModal()` 的
  一次数百毫秒级样式计算；打开后焦点回到原形态卡。
- **交互响应目标是 <50ms**（点击/触摸到第一帧可见变化）。为此交互路径上不使用实时
  `backdrop-filter`：顶栏、hero 胶囊、形态遮罩、二维码遮罩全部改为实色/高不透明度，
  回忆时光机用实色底；回忆开场动画延迟一帧启动并预载首图；形态弹窗复用卡片里**已经
  解码**的那一帧，不再临时换 `src`；three.js 在 `pointerdown`/`keydown` 后暂停 350ms，
  把主线程让给点击与首帧。
- 交互处理只做一次 rAF 批处理并缓存滚动尺寸。本机无头 Chrome（ANGLE/Intel 硬件
  WebGL，空闲）实测：表单/二维码/回忆/导航四类点击处理 0.6–18ms，首帧 9–42ms；
  移动端视口同口径 15–30ms。加载阶段 three.js 首次编译与回忆 DOM 预热是**空闲期**
  一次性长任务，不会在点击路径上执行；CSS 3D 环境层不产生长任务。
  移动端首屏传输约 119 KB（此前约 1.03 MB），进入形态区时才加载动图。
  50ms 是目标而非跨设备保证：低端机、系统繁忙、4× CPU 降速或浏览器合成器卡顿时
  仍可能超过；发布前用 Lighthouse / DevTools Performance 在目标设备复测。

## 移动端兼容

- 布局覆盖 320 / 360 / 390 / 414 / 500 / 600 / 667×375（横屏）/ 768 / 900px，
  均无横向溢出；导航和下载卡片在窄屏自动换行。
- 顶部导航触控高度 38px，右侧章节圆点扩大到 24px 热区；形态弹窗关闭键 40px。
- 短屏或横屏时时光机自动收紧标题、列表高度和底栏按钮，避免内容被裁切。
- `hover: none` 设备取消卡片悬停位移，防止触屏点击后残留“悬浮”状态。
- 头部/页脚接入 `safe-area-inset-*`，`viewport-fit=cover` 下不压刘海和手势区。
- 形态卡默认 PNG 静帧，进入附近才换 WebP 动图；不支持动图 WebP 的浏览器回退 GIF。
- 形态区已移到能力/迭代之后（页面靠后），首屏不再为十二张动图付加载优先级；导航与右侧
  章节圆点同步为 认识 → 能力 → 迭代 → 形态 → 版本 → 下载。≤460px 时导航隐藏「形态」，
  保留认识/能力/迭代/版本/下载（形态仍可滚动到）。
- 视觉：时间线改为 3×2 卡片（日期胶囊 + 情绪色顶线），能力卡加图标底片/顶线与悬停浮起，
  关于区四个特征改为小卡；都只用 CSS，不增加主线程工作。
- 320 / 390 / 768 / 横屏复查过导航换行、时间线、下载卡与形态网格；均无横向溢出。

## 部署

把 `site/` 整个目录作为静态站点根目录即可（Nginx / Caddy / Cloudflare Pages /
GitHub Pages 均可）。页面内没有构建步骤，也没有服务端依赖。

**发版时更新 `version.json`**：`{"version":"1.26.0","download":"https://opi.erik.xyz/download"}`。
（`/download` 由 `site/download/index.html` 转场到首页下载区；具体安装包在
`/download/opi-…` 下。）
小欧设置窗（Windows/Linux 桌面端）的「检测更新」先读它；读不到才回退解析
`index.html` 的 JSON-LD `softwareVersion`（再兜底下载文件名）。`version` 必须与
`Cargo.toml` / `desktop/build.gradle.kts` 的 `opiAppVersion` 同值；否则用户会被反复
提示「有新版本」或永远收不到提示。`download` 必须是 `https://`（客户端拒绝明文）。

SEO 已内置在 `index.html` 的 `<head>`：canonical、中文 description、Open Graph /
Twitter Card（绝对图片地址）、`SoftwareApplication` + `WebSite` + `Organization`
JSON-LD，以及 `robots.txt` / `sitemap.xml`。换域名时同时改 canonical、OG/Twitter
`url`/`image`、JSON-LD 的 `url`/`@id` 与 robots 的 Sitemap 地址。

- `site/README.md` 的链接若部署在子路径（如 `/opi/`），保持相对路径即可。
- 统计脚本由 busuanzi 提供；广告拦截器或离线环境会把它拦掉，页面其余部分不受影响。
- 版本号在 `index.html`（hero 的 `v1.26.0`）与发布页链接里是手写的，发版时同步。

## 素材授权

小欧与十二形态来自 `shared/pet/`，授权见仓库根目录
[`ASSETS-LICENSE.md`](../ASSETS-LICENSE.md)；three.js 为 MIT，未改动其版权头。
