// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写，**未经过任何 ArkTS 编译器**。ArkUI 的 @Component /
// Canvas / CanvasRenderingContext2D 在普通 tsc 下就断，所以这里连语法都没检查过。
//
// 项目宠物「小欧」—— 一枚键帽精灵，鸿蒙端复刻。
//
// **几何与表情的真源不是本文件**，而是：
//   * docs/opi-pet.svg          —— 静态基准（240×250 设计空间、锚点、配色释义）
//   * shared/pet/OpiPet.kt      —— 带情绪的 Compose 版（坐标已按情绪分化，本文件照抄它）
//   * android/app/src/test/kotlin/xyz/erik/opi/pet/OpiPetMoodTest.kt —— petMood 的规格
// 改小欧请同时改这三处 + 本文件；**不要在鸿蒙端另发明一只小欧**。
// 三者共享同一套几何：viewBox 240×250，所有坐标乘以缩放 u = 目标宽度 / 240。
//
// Compose → ArkUI Canvas 的两处**已知差异**（别当成 bug 去「修」）：
//   1. Compose 的 drawText 以**左上角**定位，所以 OpiPet.kt 要拿 firstBaseline 反推；
//      Canvas 的 fillText 默认以**基线**定位（textBaseline='alphabetic'），
//      SVG 的 y="187" 本来就是基线 —— 所以这里直接写 187*u，**不需要**相减。
//   2. Canvas 2D 没有 letterSpacing，所以 OPI 三个字母是逐字量宽 + 手动加字距画出来的。

/** 宠物情绪。由输入状态推导，不自己维护状态。 */
export enum PetMood {
  /** 安静待命（缓冲为空） */
  Idle = 0,
  /** 竖起天线等你挑（有候选） */
  Waiting = 1,
  /** 乱码拼音，它也没辙（有缓冲无候选） */
  Puzzled = 2,
  /** 睡着了，不记事（学习关闭） */
  Sleepy = 3,
}

/**
 * 由当前输入状态推导情绪（纯函数，无 UI 依赖）。
 *
 * **判断顺序不能改**，四条分支是 OpiPet.kt 的逐条镜像：
 * 学习关闭 → 睡着（优先于一切，连候选都不看）；
 * 缓冲为空 → 待命；有候选 → 等选；否则 → 困惑。
 *
 * 规格：android/app/src/test/kotlin/xyz/erik/opi/pet/OpiPetMoodTest.kt
 * （鸿蒙端没有可跑的测试环境，那份 Kotlin 单测就是本函数的验收标准。）
 */
export function petMood(buffer: string, candidateCount: number, learnerEnabled: boolean): PetMood {
  if (!learnerEnabled) {
    return PetMood.Sleepy;
  }
  if (buffer.length === 0) {
    return PetMood.Idle;
  }
  if (candidateCount > 0) {
    return PetMood.Waiting;
  }
  return PetMood.Puzzled;
}

/** 小欧的配色。字段与 PetPalette（OpiPet.kt）一一对应。 */
export class PetPalette {
  /** 青：结构描边 */
  primary: string;
  /** 深青：眼睛与刻字 */
  shade: string;
  /** 朱砂：声调天线（全图唯一暖色） */
  accent: string;
  /** 键帽侧壁 */
  skirt: string;
  /** 键帽正面 */
  plate: string;

  constructor(primary: string, shade: string, accent: string, skirt: string, plate: string) {
    this.primary = primary;
    this.shade = shade;
    this.accent = accent;
    this.skirt = skirt;
    this.plate = plate;
  }

  /** 浅色底（候选栏 / 设置页）。 */
  static default(): PetPalette {
    return new PetPalette('#2E8B80', '#1E6B63', '#D9553F', '#E9F3F1', '#FFFFFF');
  }

  /**
   * 深色底（对应 Windows 候选窗那种 #2D2D2D 底色）：描边与天线提亮，
   * 键帽正面压到近白但不纯白，避免在深色窗里过曝成一块死白。
   */
  static dark(): PetPalette {
    return new PetPalette('#57C9BB', '#1E6B63', '#E8705A', '#D6EAE7', '#F6FAFA');
  }
}

/** 设计空间（与 docs/opi-pet.svg 的 viewBox 相同）。 */
const CANVAS_W: number = 240;
const CANVAS_H: number = 250;
const TAU: number = Math.PI * 2;

@Component
export struct OpiPet {
  /** 情绪。变化时重画。 */
  @Prop @Watch('onVisualChange') mood: PetMood = PetMood.Idle;
  /** 键帽宽度（vp）。高度按 240:250 自动推导。 */
  @Prop @Watch('onVisualChange') size: number = 72;
  /** 深色底传 true。 */
  @Prop @Watch('onVisualChange') dark: boolean = false;
  /** 强制刻 OPI。默认 false = 按尺寸自动（≥64vp 才刻，小于此时刻字只剩几像素高、糊成脏点）。 */
  @Prop @Watch('onVisualChange') forceLegend: boolean = false;

  private settings: RenderingContextSettings = new RenderingContextSettings(true);
  private ctx: CanvasRenderingContext2D = new CanvasRenderingContext2D(this.settings);
  private ready: boolean = false;

  build() {
    Canvas(this.ctx)
      .width(this.size)
      .height(this.size * CANVAS_H / CANVAS_W)
      .onReady(() => {
        this.ready = true;
        this.draw();
      });
  }

  private onVisualChange(): void {
    // Canvas 是命令式的：状态变了不会自动重画，必须自己调。
    // onReady 之前 ctx 还没绑定画布，调用会丢，所以用 ready 挡一下。
    if (this.ready) {
      this.draw();
    }
  }

  // ---------------------------------------------------------------- 绘制

  private draw(): void {
    const ctx = this.ctx;
    const u: number = this.size / CANVAS_W; // 设计单位 → vp
    const p: PetPalette = this.dark ? PetPalette.dark() : PetPalette.default();
    const sleepy: boolean = this.mood === PetMood.Sleepy;
    const puzzled: boolean = this.mood === PetMood.Puzzled;
    const waiting: boolean = this.mood === PetMood.Waiting;
    const stroke: number = 5 * u;

    ctx.clearRect(0, 0, this.size, this.size * CANVAS_H / CANVAS_W);

    // 落地投影（SVG: ellipse cx=120 cy=206 rx=72 ry=8, opacity .08）
    ctx.fillStyle = 'rgba(34,39,46,0.08)';
    ctx.beginPath();
    ctx.ellipse(120 * u, 206 * u, 72 * u, 8 * u, 0, 0, TAU);
    ctx.fill();

    // 底部按键涟漪：键已被按下，余波只在本机扩散
    ctx.lineCap = 'round';
    ctx.lineWidth = 4 * u;
    ctx.strokeStyle = this.alpha(p.primary, 0.5);
    ctx.beginPath();
    ctx.moveTo(74 * u, 214 * u);
    ctx.quadraticCurveTo(120 * u, 228 * u, 166 * u, 214 * u);
    ctx.stroke();
    ctx.strokeStyle = this.alpha(p.primary, 0.28);
    ctx.beginPath();
    ctx.moveTo(92 * u, 226 * u);
    ctx.quadraticCurveTo(120 * u, 236 * u, 148 * u, 226 * u);
    ctx.stroke();

    // 天线：拼音声调 ˉ（一声）。睡着时整根垂下来。
    ctx.save();
    if (sleepy) {
      ctx.translate(120 * u, 48 * u);
      ctx.rotate(24 * Math.PI / 180);
      ctx.translate(-120 * u, -48 * u);
    }
    ctx.lineCap = 'round';
    ctx.strokeStyle = p.primary;
    ctx.lineWidth = stroke;
    ctx.beginPath();
    ctx.moveTo(120 * u, 52 * u);
    ctx.lineTo(120 * u, (waiting ? 24 : 28) * u);
    ctx.stroke();
    ctx.strokeStyle = p.accent;
    ctx.lineWidth = 7 * u;
    ctx.beginPath();
    ctx.moveTo(103 * u, (waiting ? 14 : 20) * u);
    ctx.lineTo(137 * u, (waiting ? 14 : 20) * u);
    ctx.stroke();
    ctx.restore();

    // 手臂：先画，接缝交给键帽正面压住。
    ctx.lineCap = 'round';
    for (let i = 0; i < 2; i++) {
      const dir: number = i === 0 ? -1 : 1;
      const ix: number = (120 + dir * 62) * u;
      const ox: number = (120 + dir * 84) * u;
      ctx.strokeStyle = p.primary;
      ctx.lineWidth = 17 * u;
      ctx.beginPath();
      ctx.moveTo(ix, 118 * u);
      ctx.lineTo(ox, 128 * u);
      ctx.stroke();
      ctx.strokeStyle = p.plate;
      ctx.lineWidth = 7 * u;
      ctx.beginPath();
      ctx.moveTo(ix, 118 * u);
      ctx.lineTo(ox, 128 * u);
      ctx.stroke();
    }

    // 键帽侧壁（上沿被正面盖住），刻 OPI。
    ctx.fillStyle = p.skirt;
    ctx.strokeStyle = p.primary;
    ctx.lineWidth = stroke;
    ctx.lineJoin = 'round';
    ctx.beginPath();
    ctx.moveTo(54 * u, 140 * u);
    ctx.lineTo(44 * u, 184 * u);
    ctx.quadraticCurveTo(42 * u, 202 * u, 62 * u, 202 * u);
    ctx.lineTo(178 * u, 202 * u);
    ctx.quadraticCurveTo(198 * u, 202 * u, 196 * u, 184 * u);
    ctx.lineTo(186 * u, 140 * u);
    ctx.closePath();
    ctx.fill();
    ctx.stroke();

    const legend: boolean = this.forceLegend || this.size >= 64;
    if (legend) {
      this.drawLegend(u, p.shade);
    }

    // 键帽正面。
    ctx.fillStyle = p.plate;
    ctx.strokeStyle = p.primary;
    ctx.lineWidth = stroke;
    this.roundRectPath(ctx, 52 * u, 46 * u, 136 * u, 104 * u, 20 * u);
    ctx.fill();
    ctx.stroke();

    // 眼睛：睡着时闭成一条线；困惑时左右不一样大。
    ctx.lineCap = 'round';
    if (sleepy) {
      ctx.strokeStyle = p.shade;
      ctx.lineWidth = stroke;
      for (let i = 0; i < 2; i++) {
        const cx: number = i === 0 ? 93 : 147;
        ctx.beginPath();
        ctx.moveTo((cx - 10) * u, 96 * u);
        ctx.lineTo((cx + 10) * u, 96 * u);
        ctx.stroke();
      }
    } else {
      const leftR: number = puzzled ? 8 : (waiting ? 12 : 11);
      const rightR: number = puzzled ? 12 : leftR;
      ctx.fillStyle = p.shade;
      ctx.beginPath();
      ctx.arc(93 * u, 95 * u, leftR * u, 0, TAU);
      ctx.fill();
      ctx.beginPath();
      ctx.arc(147 * u, 95 * u, rightR * u, 0, TAU);
      ctx.fill();
      if (!puzzled) {
        // 高光点：没了它眼睛是一坨死板的圆
        ctx.fillStyle = p.plate;
        ctx.beginPath();
        ctx.arc(89.2 * u, 91.2 * u, 3.6 * u, 0, TAU);
        ctx.fill();
        ctx.beginPath();
        ctx.arc(143.2 * u, 91.2 * u, 3.6 * u, 0, TAU);
        ctx.fill();
      }
    }

    // 嘴：圆环 = 字母 O（Open）。困惑时缩小并歪向一边。
    const mouthRx: number = puzzled ? 7 : 12;
    const mouthRy: number = puzzled ? 6 : 10;
    const mouthCx: number = puzzled ? 132 : 120;
    ctx.strokeStyle = p.shade;
    ctx.lineWidth = stroke;
    ctx.beginPath();
    ctx.ellipse(mouthCx * u, 128 * u, mouthRx * u, mouthRy * u, 0, 0, TAU);
    ctx.stroke();
  }

  /**
   * 侧壁刻字 OPI。
   *
   * Canvas 2D **没有 letterSpacing**（SVG 的 letter-spacing="4.5" 和 Compose 的
   * letterSpacing 都没有对应物），所以逐字量宽、手动累加字距，再整体居中。
   */
  private drawLegend(u: number, color: string): void {
    const ctx = this.ctx;
    const chars: string[] = ['O', 'P', 'I'];
    const gap: number = 4.5 * u;
    // 系统字体里没有 ui-monospace（那是 Apple 的）。鸿蒙没有保证存在的等宽字体，
    // 'monospace' 是通用族名，不确定是否落到 HarmonyOS Sans —— 见 README「最不确定」。
    ctx.font = `bold ${21 * u}px monospace`;
    ctx.textAlign = 'left';
    ctx.textBaseline = 'alphabetic'; // SVG 的 y=187 就是基线，直接对齐

    let total: number = 0;
    const widths: number[] = [];
    for (let i = 0; i < chars.length; i++) {
      const w: number = ctx.measureText(chars[i]).width;
      widths.push(w);
      total += w + gap;
    }
    total -= gap; // 末尾不加字距

    ctx.fillStyle = color;
    let x: number = 120 * u - total / 2;
    for (let i = 0; i < chars.length; i++) {
      ctx.fillText(chars[i], x, 187 * u);
      x += widths[i] + gap;
    }
  }

  /**
   * 圆角矩形路径（SVG 的 rect rx=20 / Compose 的 drawRoundRect 的替代）。
   * Canvas 2D 的 roundRect 在 ArkUI 上不保证存在，用 arcTo 四条边拼出来更稳。
   */
  private roundRectPath(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number,
    r: number): void {
    ctx.beginPath();
    ctx.moveTo(x + r, y);
    ctx.lineTo(x + w - r, y);
    ctx.arcTo(x + w, y, x + w, y + r, r);
    ctx.lineTo(x + w, y + h - r);
    ctx.arcTo(x + w, y + h, x + w - r, y + h, r);
    ctx.lineTo(x + r, y + h);
    ctx.arcTo(x, y + h, x, y + h - r, r);
    ctx.lineTo(x, y + r);
    ctx.arcTo(x, y, x + r, y, r);
    ctx.closePath();
  }

  /** '#RRGGBB' + alpha → 'rgba(r,g,b,a)'。SVG 的 opacity 在 Canvas 上只能这样表达。 */
  private alpha(hex: string, a: number): string {
    const r: number = parseInt(hex.substring(1, 3), 16);
    const g: number = parseInt(hex.substring(3, 5), 16);
    const b: number = parseInt(hex.substring(5, 7), 16);
    return `rgba(${r},${g},${b},${a})`;
  }
}
