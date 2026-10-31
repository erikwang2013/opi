// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! Named-pipe NDJSON client for `\\.\pipe\opi-v2`.
//!
//! Phase 1 consumes `show`/`hide`/`position` frames from a server endpoint and
//! reconnects after the peer goes away. The read lives on its own thread so a
//! blocking pipe read never stalls the Win32 message loop.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};

use crate::protocol::PIPE_NAME;

pub struct PipeClient {
    reader: BufReader<File>,
}

impl PipeClient {
    /// One connect attempt; the caller owns the retry loop.
    pub fn connect() -> std::io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(PIPE_NAME)?;
        Ok(Self {
            reader: BufReader::new(file),
        })
    }

    pub fn read_line(&mut self, out: &mut String) -> std::io::Result<usize> {
        self.reader.read_line(out)
    }

    /// Send one frame (newline added). Phase 2 uses this for select/page/commit.
    #[allow(dead_code)]
    pub fn send_line(&mut self, line: &str) -> std::io::Result<()> {
        let file = self.reader.get_mut();
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
        file.flush()
    }
}
