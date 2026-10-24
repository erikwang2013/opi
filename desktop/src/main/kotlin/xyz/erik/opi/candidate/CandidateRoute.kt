// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 候选管道的**版本隔离 + 消息取舍**（2026-10-25 从 PipeServer.kt 拆出：那边
//! 加双管道后正好 500 行，门禁无余量）。纯常量/纯函数，Linux 可单测；管道读写
//! 仍在 PipeServer.kt。

package xyz.erik.opi.candidate

/**
 * 新版 TSF 客户端用的管道（与 Rust 侧 candidate_io.rs 的 PIPE_NAME 一致）。
 * 旧版 DLL 只会连 [LEGACY_PIPE_NAME]；新版走本管道，候选窗进程两条都服务。
 */
internal const val PRIMARY_PIPE_NAME = "\\\\.\\pipe\\opi-v2"

/**
 * 旧版 TSF 客户端用的管道名，**永不改**。已打开的应用里仍映射着旧 DLL，
 * 它们只认这个名字；新版候选窗的 legacy 服务端继续为它们提供候选。
 */
internal const val LEGACY_PIPE_NAME = "\\\\.\\pipe\\opi"

/** show 消息来自哪条管道；同一时刻只有一条能驱动候选窗内容。 */
internal enum class CandidateSource { NONE, PRIMARY, LEGACY }

/**
 * 该不该接受这条 `show`：PRIMARY 永远优先；LEGACY 只在 PRIMARY 没在显示时接受。
 * 纯函数，Linux 可单测（见 CandidatePipeRoutingTest）。
 */
internal fun acceptsShow(current: CandidateSource, from: CandidateSource): Boolean =
    from == CandidateSource.PRIMARY || current != CandidateSource.PRIMARY

/**
 * 管道断开时该不该清掉候选窗：
 * · PRIMARY 断开：只在当前显示的不是 LEGACY 时清（PRIMARY 接管过才清）；
 *   新应用只是激活、没打过字时，不能把旧应用正显示的候选一起藏掉。
 * · LEGACY 断开：只在当前显示的就是它时清。
 */
internal fun hidesModel(current: CandidateSource, from: CandidateSource): Boolean = when (from) {
    CandidateSource.PRIMARY -> current != CandidateSource.LEGACY
    CandidateSource.LEGACY -> current == CandidateSource.LEGACY
    CandidateSource.NONE -> false
}
