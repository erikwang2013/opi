// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 候选窗出站队列（平台中立）：有界、保序、单消费者，溢出丢弃与关闭语义有主机单测。
//!
//! Windows 的 TSF 按键线程只调用 [`SendQueue::enqueue`]（一次短锁，绝不碰管道）；
//! 唯一的 writer 线程阻塞在 [`SendQueue::pop`] 上，负责连接与 `WriteFile`。
//! 候选窗进程卡死时，按键线程最多入队失败，不会卡在无超时的管道写上
//! （2026-11 性能项 4）。
//!
//! 溢出策略（队列满时）：**新消息优先于旧状态**，最终 UI 由最新状态帧决定。
//! 依次淘汰：最旧的 `Position` → 最旧的 `State` → 最旧的 `Control`；
//! 新 `Position` 在无旧位置可淘汰时直接丢弃（不挤掉状态/控制帧）；
//! 新 `Control` 不挤掉 `State`。保留下来的消息相对顺序不变。
//!
//! 关闭：[`SendQueue::close`] 后 `enqueue` 返回 false、`pop` 立刻返回 None
//! （未发送的积压直接丢弃，不再写旧状态），用于客户端 Drop / 卸载。

use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};

/// 队列默认容量：一帧 show 几 KB，64 帧至多数百 KB；正常按键远达不到。
pub const DEFAULT_CAPACITY: usize = 64;

/// 消息类别只影响**溢出时淘汰谁**，不改变线协议字节。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendClass {
    /// caret 位置：幂等、过期最快，溢出时最先淘汰。
    Position,
    /// show/hide/state_changed：保留最新一条即可保证 UI 不停在旧状态。
    State,
    /// ctl_result 应答：尽量保留（对端按 id 等应答），但绝不挤掉 State。
    Control,
}

/// 一条待发送的完整帧（含结尾 '\n'，线协议见 candidate_io.rs）。
#[derive(Debug)]
pub struct Outbound {
    pub class: SendClass,
    pub bytes: Vec<u8>,
}

struct Inner {
    items: VecDeque<Outbound>,
    closed: bool,
}

/// 有界队列：多线程入队，单消费者出队。入队不等待 I/O，只等短锁。
pub struct SendQueue {
    capacity: usize,
    inner: Mutex<Inner>,
    wake: Condvar,
}

impl SendQueue {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        assert!(capacity > 0, "candidate queue capacity must be > 0");
        Self {
            capacity,
            inner: Mutex::new(Inner {
                items: VecDeque::new(),
                closed: false,
            }),
            wake: Condvar::new(),
        }
    }

    /// 非阻塞入队（仅短锁，不等待 writer/管道）。返回 false = 队列已关闭。
    pub fn enqueue(&self, class: SendClass, bytes: Vec<u8>) -> bool {
        let Ok(mut inner) = self.inner.lock() else {
            return false; // 锁中毒：与 tsf.rs / candidate_io.rs 的锁策略一致，吞掉
        };
        if inner.closed {
            return false;
        }
        if inner.items.len() >= self.capacity && !inner.make_room(class) {
            // 队列满且没有值得淘汰的旧帧：丢弃这条新消息（只可能是 Position/Control）。
            return true;
        }
        inner.items.push_back(Outbound { class, bytes });
        drop(inner);
        self.wake.notify_one();
        true
    }

    /// 阻塞取下一条；队列关闭后返回 None（即使还有积压）。
    pub fn pop(&self) -> Option<Outbound> {
        let Ok(mut inner) = self.inner.lock() else {
            return None;
        };
        loop {
            if inner.closed {
                return None;
            }
            if let Some(item) = inner.items.pop_front() {
                return Some(item);
            }
            inner = self.wake.wait(inner).ok()?;
        }
    }

    /// 关闭队列：唤醒 writer，拒绝后续入队；未发送的积压不再发送。
    pub fn close(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.closed = true;
            inner.items.clear();
        }
        self.wake.notify_all();
    }

    /// 丢弃所有积压但保持队列可用：宿主失活时清掉旧状态，重新激活后不重放旧页。
    pub fn clear(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.items.clear();
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.inner.lock().map(|g| g.items.len()).unwrap_or(0)
    }
}

impl Default for SendQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl Inner {
    /// 队列已满：按类别淘汰一条旧帧。返回 true = 腾出了位置。
    fn make_room(&mut self, incoming: SendClass) -> bool {
        fn first(items: &mut VecDeque<Outbound>, class: SendClass) -> bool {
            items
                .iter()
                .position(|m| m.class == class)
                .map(|i| items.remove(i))
                .is_some()
        }
        match incoming {
            SendClass::Position => first(&mut self.items, SendClass::Position),
            SendClass::State => {
                first(&mut self.items, SendClass::Position)
                    || first(&mut self.items, SendClass::State)
                    || first(&mut self.items, SendClass::Control)
            }
            SendClass::Control => {
                first(&mut self.items, SendClass::Position)
                    || first(&mut self.items, SendClass::Control)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    use super::*;

    fn msg(class: SendClass, text: &str) -> (SendClass, Vec<u8>) {
        (class, text.as_bytes().to_vec())
    }

    fn drain(q: &SendQueue) -> Vec<(SendClass, String)> {
        let mut out = Vec::new();
        for _ in 0..q.len() {
            let item = q.pop().expect("队列非空，pop 不应阻塞");
            out.push((item.class, String::from_utf8(item.bytes).unwrap()));
        }
        out
    }

    #[test]
    fn fifo_preserves_order_across_classes() {
        let q = SendQueue::with_capacity(8);
        for (class, bytes) in [
            msg(SendClass::Position, "pos"),
            msg(SendClass::State, "show"),
            msg(SendClass::Control, "ctl"),
            msg(SendClass::State, "hide"),
        ] {
            assert!(q.enqueue(class, bytes));
        }
        assert_eq!(
            drain(&q),
            vec![
                (SendClass::Position, "pos".into()),
                (SendClass::State, "show".into()),
                (SendClass::Control, "ctl".into()),
                (SendClass::State, "hide".into()),
            ]
        );
    }

    #[test]
    fn overflow_drops_oldest_position_first() {
        let q = SendQueue::with_capacity(2);
        q.enqueue(SendClass::Position, b"p1".to_vec());
        q.enqueue(SendClass::State, b"s1".to_vec());
        q.enqueue(SendClass::Position, b"p2".to_vec());
        assert_eq!(
            drain(&q),
            vec![
                (SendClass::State, "s1".into()),
                (SendClass::Position, "p2".into()),
            ]
        );
    }

    #[test]
    fn overflow_keeps_latest_state_in_order() {
        let q = SendQueue::with_capacity(2);
        q.enqueue(SendClass::State, b"s1".to_vec());
        q.enqueue(SendClass::State, b"s2".to_vec());
        q.enqueue(SendClass::State, b"s3".to_vec());
        assert_eq!(
            drain(&q),
            vec![
                (SendClass::State, "s2".into()),
                (SendClass::State, "s3".into()),
            ]
        );
    }

    #[test]
    fn hide_survives_show_flood() {
        let q = SendQueue::with_capacity(2);
        q.enqueue(SendClass::State, b"show-old".to_vec());
        q.enqueue(SendClass::State, b"show-new".to_vec());
        q.enqueue(SendClass::State, b"hide".to_vec());
        let items = drain(&q);
        assert_eq!(items.last().unwrap().1, "hide");
        assert_eq!(
            items,
            vec![
                (SendClass::State, "show-new".into()),
                (SendClass::State, "hide".into()),
            ]
        );
    }

    #[test]
    fn new_position_never_evicts_state_or_control() {
        let q = SendQueue::with_capacity(2);
        q.enqueue(SendClass::State, b"s1".to_vec());
        q.enqueue(SendClass::State, b"s2".to_vec());
        q.enqueue(SendClass::Position, b"p".to_vec());
        assert_eq!(
            drain(&q),
            vec![
                (SendClass::State, "s1".into()),
                (SendClass::State, "s2".into()),
            ]
        );

        let q = SendQueue::with_capacity(1);
        q.enqueue(SendClass::Control, b"ctl".to_vec());
        q.enqueue(SendClass::Position, b"p".to_vec());
        assert_eq!(drain(&q), vec![(SendClass::Control, "ctl".into())]);
    }

    #[test]
    fn state_evicts_control_but_control_never_evicts_state() {
        let q = SendQueue::with_capacity(1);
        q.enqueue(SendClass::Control, b"ctl".to_vec());
        q.enqueue(SendClass::State, b"show".to_vec());
        assert_eq!(drain(&q), vec![(SendClass::State, "show".into())]);

        let q = SendQueue::with_capacity(1);
        q.enqueue(SendClass::State, b"show".to_vec());
        q.enqueue(SendClass::Control, b"ctl".to_vec());
        assert_eq!(drain(&q), vec![(SendClass::State, "show".into())]);
    }

    #[test]
    fn close_wakes_blocked_pop_and_rejects_enqueue() {
        let q = Arc::new(SendQueue::with_capacity(4));
        let waiter = {
            let q = Arc::clone(&q);
            thread::spawn(move || q.pop())
        };
        thread::sleep(Duration::from_millis(20));
        q.close();
        assert!(waiter.join().unwrap().is_none());
        assert!(!q.enqueue(SendClass::State, b"late".to_vec()));
        assert!(q.pop().is_none());
    }

    #[test]
    fn close_discards_pending_and_clear_keeps_queue_usable() {
        let q = SendQueue::with_capacity(4);
        q.enqueue(SendClass::State, b"old".to_vec());
        q.clear();
        q.enqueue(SendClass::State, b"new".to_vec());
        assert_eq!(drain(&q), vec![(SendClass::State, "new".into())]);

        q.enqueue(SendClass::State, b"pending".to_vec());
        q.close();
        assert!(q.pop().is_none(), "关闭后未发送的积压必须丢弃");
    }
}
