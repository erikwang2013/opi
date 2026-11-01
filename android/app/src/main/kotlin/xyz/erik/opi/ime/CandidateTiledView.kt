// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.pager.VerticalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import xyz.erik.opi.candidate.ROWS_PER_PAGE
import xyz.erik.opi.candidate.UNITS_PER_ROW
import xyz.erik.opi.candidate.tilePages
import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.keyboard.CandidateReadingColumn
import xyz.erik.opi.keyboard.KeyButton
import xyz.erik.opi.keyboard.LocalKeyboardPalette

/** 平铺视图左列的格数（与键盘左列同节奏：五格）。 */
private const val TILED_READING_CELLS = 5

/**
 * 读数列格数（3–5）：原固定 5 格只在竖屏成立 —— 横屏键区总高约 185dp，5 格每格 37dp，
 * 26sp 读数在长辈模式 ×1.18 后行高约 38dp，上下各裁一点。按可用高度降到 4/3 格，
 * 每格保持 ≥44dp（常规字号行高约 31dp + 余量）。
 */
internal fun tiledReadingCells(availableHeightDp: Float): Int = when {
    availableHeightDp < 160f -> 3
    availableHeightDp < 220f -> 4
    else -> TILED_READING_CELLS
}

/**
 * **平铺候选视图**（2026-10-16 用户裁决「左右翻页改成替换键盘平铺出来……上下滑动翻页」，
 * 布局对齐参考图 2）：占满「候选｜工具条」共用格与整个键盘区 ——
 * · 左列 = 放大的读音列（同 [CandidateReadingColumn]：点行 = 钉读音）；
 * · 中间 = 候选按 [tilePages] 平铺（短词一行 4 个、长词一行 2 个），**上下滑动翻页**；
 * · 右列 = 返回 / ⌫ / 重输 / 空格（⌫/重输/空格与键盘右列同观感，返回是平铺态唯一的
 *   收拢入口 —— 输入中工具条被候选顶掉，没有别的出口）。
 *
 * 收拢规则：返回 = [ImeState.collapseCandidates]；选词/⌫/重输/空格之后一律调
 * [ImeState.collapseIfNoCandidates]（候选没了就自动回键盘，否则键盘回不来）。
 * 状态机在 `ImeStateTiledTest`，打包规则在 `TilePackTest`（渲染以模拟器截屏核验）。
 */
@Composable
fun CandidateTiledView(
    state: ImeState,
    controller: EngineController,
    router: KeyRouter,
    modifier: Modifier = Modifier,
) {
    val p = LocalKeyboardPalette.current
    val cands = controller.candidates
    val pages = remember(cands) { tilePages(cands) }
    val pager = rememberPagerState(pageCount = { pages.size.coerceAtLeast(1) })
    // 候选整批换了（继续输入 / 钉读音 / 选词后剩尾巴接着编）⇒ 回第一页，别停在旧页号上。
    LaunchedEffect(cands) { pager.scrollToPage(0) }
    BoxWithConstraints(modifier = modifier.fillMaxSize().opiPanelBackground(p.barBg)) {
        // 横屏/分屏高度不够时减少读数列格数，见 [tiledReadingCells]。
        val readingCells = tiledReadingCells(maxHeight.value)
        Row(modifier = Modifier.fillMaxSize()) {
            // 左列：放大的读音列 —— 与键盘左列**同一块** CandidateReadingColumn（点行 = 钉
            // 读音、读屏串、滚动复位都逐字同构，2026-10-16 抽出）。
            CandidateReadingColumn(
                readings = controller.candidateReadings,
                pinnedReading = controller.pinnedReading,
                onPick = { i ->
                    controller.candidateReadings.getOrNull(i)?.let(controller::togglePinnedReading)
                },
                scrollKey = controller.buffer,
                cells = readingCells,
                fontSize = 26.sp,
                modifier = Modifier.weight(1f).fillMaxHeight(),
            )
            VerticalPager(
                state = pager,
                modifier = Modifier.weight(3f).fillMaxHeight(),
            ) { page ->
                Column(modifier = Modifier.fillMaxSize()) {
                    val rows = pages.getOrNull(page).orEmpty()
                    repeat(ROWS_PER_PAGE) { r ->
                        Row(modifier = Modifier.weight(1f).fillMaxWidth()) {
                            val row = rows.getOrNull(r).orEmpty()
                            var used = 0
                            for (cell in row) {
                                used += cell.units
                                TiledCell(
                                    text = cands.getOrElse(cell.index) { "" },
                                    onTap = {
                                        router.handleCandidateAbsolute(cell.index)
                                        state.collapseIfNoCandidates()
                                    },
                                    modifier = Modifier.weight(cell.units.toFloat()),
                                )
                            }
                            // 补空位：本行剩余单位用空 Box 占掉，否则 weight 会把实格拉宽。
                            if (used < UNITS_PER_ROW) {
                                Box(modifier = Modifier.weight((UNITS_PER_ROW - used).toFloat()))
                            }
                        }
                    }
                }
            }
            // 右列：返回 + 与键盘右列同三键（⌫ / 重输 / 空格）。
            Column(modifier = Modifier.weight(1f).fillMaxHeight()) {
                KeyButton("返回", modifier = Modifier.weight(1f).fillMaxWidth(), onTap = {
                    state.collapseCandidates()
                })
                KeyButton(
                    "⌫",
                    modifier = Modifier.weight(1f).fillMaxWidth(),
                    onTap = {
                        router.handleBackspace()
                        state.collapseIfNoCandidates()
                    },
                    // 按住连续删除（2026-10-23 用户裁决）；候选删空会收拢回键盘，
                    // 键随之卸载、重复手势自然停止。
                    repeatOnLongPress = true,
                )
                KeyButton("重输", modifier = Modifier.weight(1f).fillMaxWidth(), onTap = {
                    controller.clear()
                    state.collapseIfNoCandidates()
                })
                KeyButton(
                    "空格",
                    modifier = Modifier.weight(1f).fillMaxWidth(),
                    onTap = {
                        router.handleSpace()
                        state.collapseIfNoCandidates()
                    },
                    mic = true,
                )
            }
        }
    }
}

/**
 * 平铺的一格：只画候选大字。
 * 2026-10-17 随折叠行同步去掉「序号小字」（用户裁决「移动端候选词不要前面的数字」，
 * 见 CandidateBar 顶注）—— 两个视图要还是同一形态，去掉也得一起去。
 */
@Composable
private fun TiledCell(
    text: String,
    onTap: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val p = LocalKeyboardPalette.current
    Box(
        modifier = modifier
            .fillMaxHeight()
            .padding(1.dp)
            .clip(RoundedCornerShape(6.dp))
            .background(p.keyBg)
            .clickable(onClick = onTap)
            .semantics {
                role = Role.Button
                contentDescription = text
            },
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text,
            fontSize = 22.sp,
            color = p.keyFg,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}
