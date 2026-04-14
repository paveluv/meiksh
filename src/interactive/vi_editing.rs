use crate::bstr::{self, ByteWriter};
use crate::shell::Shell;
use crate::sys;

struct RawMode {
    saved: libc::termios,
}

impl RawMode {
    fn enter() -> sys::SysResult<Self> {
        let saved = sys::get_terminal_attrs(sys::STDIN_FILENO)?;
        let mut raw = saved;
        raw.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG);
        raw.c_cc[libc::VMIN] = 1;
        raw.c_cc[libc::VTIME] = 0;
        sys::set_terminal_attrs(sys::STDIN_FILENO, &raw)?;
        Ok(Self { saved })
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = sys::set_terminal_attrs(sys::STDIN_FILENO, &self.saved);
    }
}

fn read_byte() -> sys::SysResult<Option<u8>> {
    let mut buf = [0u8; 1];
    match sys::read_fd(sys::STDIN_FILENO, &mut buf) {
        Ok(0) => Ok(None),
        Ok(_) => Ok(Some(buf[0])),
        Err(e) => Err(e),
    }
}

fn write_bytes(data: &[u8]) {
    let _ = sys::write_all_fd(sys::STDOUT_FILENO, data);
}

fn bell() {
    write_bytes(b"\x07");
}

fn redraw(line: &[u8], cursor: usize, prompt: &[u8]) {
    write_bytes(b"\r\x1b[K");
    let _ = sys::write_all_fd(sys::STDERR_FILENO, prompt);
    let mut buf = Vec::with_capacity(line.len() + 20);
    buf.extend_from_slice(line);
    let cursor_back = line.len().saturating_sub(cursor);
    if cursor_back > 0 {
        buf.extend_from_slice(b"\x1b[");
        bstr::push_u64(&mut buf, cursor_back as u64);
        buf.push(b'D');
    }
    write_bytes(&buf);
}

pub(crate) fn is_word_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

pub(crate) fn word_forward(line: &[u8], pos: usize) -> usize {
    let mut p = pos;
    let len = line.len();
    if p >= len {
        return p;
    }
    if is_word_char(line[p]) {
        while p < len && is_word_char(line[p]) {
            p += 1;
        }
    } else if !line[p].is_ascii_whitespace() {
        while p < len && !is_word_char(line[p]) && !line[p].is_ascii_whitespace() {
            p += 1;
        }
    }
    while p < len && line[p].is_ascii_whitespace() {
        p += 1;
    }
    p
}

pub(crate) fn word_backward(line: &[u8], pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    let mut p = pos;
    while p > 0 && line[p - 1].is_ascii_whitespace() {
        p -= 1;
    }
    if p == 0 {
        return 0;
    }
    if is_word_char(line[p - 1]) {
        while p > 0 && is_word_char(line[p - 1]) {
            p -= 1;
        }
    } else {
        while p > 0 && !is_word_char(line[p - 1]) && !line[p - 1].is_ascii_whitespace() {
            p -= 1;
        }
    }
    p
}

pub(crate) fn bigword_forward(line: &[u8], pos: usize) -> usize {
    let mut p = pos;
    let len = line.len();
    while p < len && !line[p].is_ascii_whitespace() {
        p += 1;
    }
    while p < len && line[p].is_ascii_whitespace() {
        p += 1;
    }
    p
}

pub(crate) fn bigword_backward(line: &[u8], pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    let mut p = pos;
    while p > 0 && line[p - 1].is_ascii_whitespace() {
        p -= 1;
    }
    while p > 0 && !line[p - 1].is_ascii_whitespace() {
        p -= 1;
    }
    p
}

pub(crate) fn word_end(line: &[u8], pos: usize) -> usize {
    let len = line.len();
    if pos + 1 >= len {
        return pos;
    }
    let mut p = pos + 1;
    while p < len && line[p].is_ascii_whitespace() {
        p += 1;
    }
    if p >= len {
        return len.saturating_sub(1);
    }
    if is_word_char(line[p]) {
        while p + 1 < len && is_word_char(line[p + 1]) {
            p += 1;
        }
    } else {
        while p + 1 < len && !is_word_char(line[p + 1]) && !line[p + 1].is_ascii_whitespace() {
            p += 1;
        }
    }
    p
}

pub(crate) fn bigword_end(line: &[u8], pos: usize) -> usize {
    let len = line.len();
    if pos + 1 >= len {
        return pos;
    }
    let mut p = pos + 1;
    while p < len && line[p].is_ascii_whitespace() {
        p += 1;
    }
    if p >= len {
        return len.saturating_sub(1);
    }
    while p + 1 < len && !line[p + 1].is_ascii_whitespace() {
        p += 1;
    }
    p
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ViAction {
    Redraw,
    Bell,
    Return(Option<Vec<u8>>),
    ReadByte,
    WriteBytes(Vec<u8>),
    RunEditor { editor: Vec<u8>, tmp_path: Vec<u8> },
    NeedSearchByte,
    NeedFindTarget,
    NeedReplaceChar,
    NeedMotion,
    NeedReplaceModeInput,
    NeedLiteralChar,
    SetInsertMode(bool),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PendingInput {
    None,
    CountDigits,
    FindTarget { cmd: u8, count: usize },
    ReplaceChar { count: usize },
    ReplaceMode,
    Motion { op: u8, count: usize },
    LiteralChar,
    SearchInput { direction: u8 },
}

pub(crate) struct ViState {
    pub line: Vec<u8>,
    pub cursor: usize,
    pub insert_mode: bool,
    pub yank_buf: Vec<u8>,
    pub last_cmd: Option<(u8, usize, Option<u8>)>,
    pub last_find: Option<(u8, u8)>,
    pub hist_index: Option<usize>,
    pub edit_line: Vec<u8>,
    pub search_buf: Vec<u8>,
    pub count_buf: Option<(usize, u8)>,
    pub pending: PendingInput,
    erase_char: u8,
    hist_len: usize,
}

impl ViState {
    pub(crate) fn new(erase_char: u8, hist_len: usize) -> Self {
        Self {
            line: Vec::new(),
            cursor: 0,
            insert_mode: true,
            yank_buf: Vec::new(),
            last_cmd: None,
            last_find: None,
            hist_index: None,
            edit_line: Vec::new(),
            search_buf: Vec::new(),
            count_buf: None,
            pending: PendingInput::None,
            erase_char,
            hist_len,
        }
    }

    pub(crate) fn process_byte(&mut self, byte: u8, history: &[Box<[u8]>]) -> Vec<ViAction> {
        let mut actions = Vec::new();

        match &self.pending {
            PendingInput::CountDigits => {
                if byte.is_ascii_digit() {
                    if let Some((ref mut count, _)) = self.count_buf {
                        *count = count
                            .saturating_mul(10)
                            .saturating_add((byte - b'0') as usize);
                    }
                    return vec![ViAction::ReadByte];
                }
                let (count, first_byte) = self.count_buf.take().unwrap();
                self.pending = PendingInput::None;
                return self.process_command(byte, count, first_byte, history);
            }
            PendingInput::FindTarget { cmd, count } => {
                let cmd = *cmd;
                let count = *count;
                self.pending = PendingInput::None;
                self.last_find = Some((cmd, byte));
                for _ in 0..count {
                    if let Some(pos) = do_find(&self.line, self.cursor, cmd, byte) {
                        self.cursor = pos;
                    } else {
                        actions.push(ViAction::Bell);
                        break;
                    }
                }
                actions.push(ViAction::Redraw);
                return actions;
            }
            PendingInput::ReplaceChar { count } => {
                let count = *count;
                self.pending = PendingInput::None;
                self.last_cmd = Some((b'r', count, Some(byte)));
                for _ in 0..count {
                    if self.cursor < self.line.len() {
                        self.line[self.cursor] = byte;
                        if self.cursor + 1 < self.line.len() {
                            self.cursor += 1;
                        }
                    }
                }
                if count > 1 && self.cursor > 0 {
                    self.cursor -= 1;
                }
                actions.push(ViAction::Redraw);
                return actions;
            }
            PendingInput::ReplaceMode => match byte {
                0x1b => {
                    self.pending = PendingInput::None;
                    if self.cursor > 0 && self.cursor >= self.line.len() {
                        self.cursor = self.line.len().saturating_sub(1);
                    }
                    actions.push(ViAction::Redraw);
                    return actions;
                }
                b'\r' | b'\n' => {
                    self.pending = PendingInput::None;
                    let mut s = self.line.clone();
                    s.push(b'\n');
                    return vec![
                        ViAction::WriteBytes(b"\r\n".to_vec()),
                        ViAction::Return(Some(s)),
                    ];
                }
                b => {
                    if self.cursor < self.line.len() {
                        self.line[self.cursor] = b;
                    } else {
                        self.line.push(b);
                    }
                    self.cursor += 1;
                    actions.push(ViAction::Redraw);
                    return actions;
                }
            },
            PendingInput::Motion { op, count } => {
                let op = *op;
                let count = *count;
                self.pending = PendingInput::None;
                return self.process_motion(op, byte, count, &mut actions);
            }
            PendingInput::LiteralChar => {
                self.pending = PendingInput::None;
                self.line.insert(self.cursor, byte);
                self.cursor += 1;
                actions.push(ViAction::Redraw);
                return actions;
            }
            PendingInput::SearchInput { direction } => {
                let direction = *direction;
                match byte {
                    b'\r' | b'\n' => {
                        self.pending = PendingInput::None;
                        if !self.search_buf.is_empty() {
                            self.do_search(direction, history, &mut actions);
                        }
                        actions.push(ViAction::Redraw);
                        return actions;
                    }
                    0x7f | 0x08 => {
                        if !self.search_buf.is_empty() {
                            self.search_buf.pop();
                            actions.push(ViAction::WriteBytes(b"\x08 \x08".to_vec()));
                        }
                        return actions;
                    }
                    b => {
                        self.search_buf.push(b);
                        actions.push(ViAction::WriteBytes(vec![b]));
                        return actions;
                    }
                }
            }
            PendingInput::None => {}
        }

        if self.insert_mode {
            match byte {
                0x1b => {
                    self.insert_mode = false;
                    if self.cursor > 0 && self.cursor >= self.line.len() {
                        self.cursor = self.line.len().saturating_sub(1);
                        actions.push(ViAction::WriteBytes(b"\x1b[D".to_vec()));
                    }
                }
                b'\n' | b'\r' => {
                    let mut s = self.line.clone();
                    s.push(b'\n');
                    actions.push(ViAction::WriteBytes(b"\r\n".to_vec()));
                    actions.push(ViAction::Return(Some(s)));
                }
                0x16 => {
                    self.pending = PendingInput::LiteralChar;
                    actions.push(ViAction::NeedLiteralChar);
                }
                0x17 => {
                    if self.cursor > 0 {
                        let start = word_backward(&self.line, self.cursor);
                        self.line.drain(start..self.cursor);
                        self.cursor = start;
                        actions.push(ViAction::Redraw);
                    }
                }
                0x03 => {
                    actions.push(ViAction::WriteBytes(b"\r\n".to_vec()));
                    actions.push(ViAction::Return(Some(Vec::new())));
                }
                0x04 => {
                    if self.line.is_empty() {
                        actions.push(ViAction::WriteBytes(b"\r\n".to_vec()));
                        actions.push(ViAction::Return(None));
                    }
                }
                b if b == self.erase_char || b == 0x7f || b == 0x08 => {
                    if self.cursor > 0 {
                        self.cursor -= 1;
                        self.line.remove(self.cursor);
                        actions.push(ViAction::Redraw);
                    }
                }
                _ => {
                    self.line.insert(self.cursor, byte);
                    self.cursor += 1;
                    if self.cursor == self.line.len() {
                        actions.push(ViAction::WriteBytes(vec![byte]));
                    } else {
                        actions.push(ViAction::Redraw);
                    }
                }
            }
            return actions;
        }

        if byte.is_ascii_digit() && byte != b'0' {
            let count = (byte - b'0') as usize;
            self.count_buf = Some((count, byte));
            self.pending = PendingInput::CountDigits;
            return vec![ViAction::ReadByte];
        }

        self.process_command(byte, 1, byte, history)
    }

    fn process_command(
        &mut self,
        ch: u8,
        count: usize,
        first_byte: u8,
        history: &[Box<[u8]>],
    ) -> Vec<ViAction> {
        let mut actions = Vec::new();

        match ch {
            b'i' => {
                self.insert_mode = true;
                actions.push(ViAction::SetInsertMode(true));
            }
            b'a' => {
                self.insert_mode = true;
                if !self.line.is_empty() {
                    self.cursor = (self.cursor + 1).min(self.line.len());
                    actions.push(ViAction::Redraw);
                }
                actions.push(ViAction::SetInsertMode(true));
            }
            b'A' => {
                self.insert_mode = true;
                self.cursor = self.line.len();
                actions.push(ViAction::Redraw);
                actions.push(ViAction::SetInsertMode(true));
            }
            b'I' => {
                self.insert_mode = true;
                self.cursor = 0;
                actions.push(ViAction::Redraw);
                actions.push(ViAction::SetInsertMode(true));
            }
            b'h' => {
                let n = count.min(self.cursor);
                self.cursor -= n;
                if n > 0 {
                    let esc = ByteWriter::new()
                        .bytes(b"\x1b[")
                        .usize_val(n)
                        .byte(b'D')
                        .finish();
                    actions.push(ViAction::WriteBytes(esc));
                } else {
                    actions.push(ViAction::Bell);
                }
            }
            b'l' | b' ' => {
                let max = self.line.len().saturating_sub(1);
                let n = count.min(max.saturating_sub(self.cursor));
                self.cursor += n;
                if n > 0 {
                    let esc = ByteWriter::new()
                        .bytes(b"\x1b[")
                        .usize_val(n)
                        .byte(b'C')
                        .finish();
                    actions.push(ViAction::WriteBytes(esc));
                } else {
                    actions.push(ViAction::Bell);
                }
            }
            b'0' if first_byte == b'0' => {
                self.cursor = 0;
                actions.push(ViAction::Redraw);
            }
            b'$' => {
                if !self.line.is_empty() {
                    self.cursor = self.line.len() - 1;
                }
                actions.push(ViAction::Redraw);
            }
            b'^' => {
                self.cursor = self
                    .line
                    .iter()
                    .position(|c| !c.is_ascii_whitespace())
                    .unwrap_or(0);
                actions.push(ViAction::Redraw);
            }
            b'w' => {
                for _ in 0..count {
                    let next = word_forward(&self.line, self.cursor);
                    if next == self.cursor {
                        actions.push(ViAction::Bell);
                        break;
                    }
                    self.cursor = next.min(self.line.len().saturating_sub(1));
                }
                actions.push(ViAction::Redraw);
            }
            b'W' => {
                for _ in 0..count {
                    let next = bigword_forward(&self.line, self.cursor);
                    if next == self.cursor {
                        actions.push(ViAction::Bell);
                        break;
                    }
                    self.cursor = next.min(self.line.len().saturating_sub(1));
                }
                actions.push(ViAction::Redraw);
            }
            b'b' => {
                for _ in 0..count {
                    let prev = word_backward(&self.line, self.cursor);
                    if prev == self.cursor {
                        actions.push(ViAction::Bell);
                        break;
                    }
                    self.cursor = prev;
                }
                actions.push(ViAction::Redraw);
            }
            b'B' => {
                for _ in 0..count {
                    let prev = bigword_backward(&self.line, self.cursor);
                    if prev == self.cursor {
                        actions.push(ViAction::Bell);
                        break;
                    }
                    self.cursor = prev;
                }
                actions.push(ViAction::Redraw);
            }
            b'e' => {
                for _ in 0..count {
                    self.cursor = word_end(&self.line, self.cursor);
                }
                actions.push(ViAction::Redraw);
            }
            b'E' => {
                for _ in 0..count {
                    self.cursor = bigword_end(&self.line, self.cursor);
                }
                actions.push(ViAction::Redraw);
            }
            b'|' => {
                let col = count
                    .saturating_sub(1)
                    .min(self.line.len().saturating_sub(1));
                self.cursor = col;
                actions.push(ViAction::Redraw);
            }
            b'f' | b'F' | b't' | b'T' => {
                self.pending = PendingInput::FindTarget { cmd: ch, count };
                actions.push(ViAction::NeedFindTarget);
            }
            b';' => {
                if let Some((cmd, target)) = self.last_find {
                    for _ in 0..count {
                        if let Some(pos) = do_find(&self.line, self.cursor, cmd, target) {
                            self.cursor = pos;
                        } else {
                            actions.push(ViAction::Bell);
                            break;
                        }
                    }
                    actions.push(ViAction::Redraw);
                }
            }
            b',' => {
                if let Some((cmd, target)) = self.last_find {
                    let rev = match cmd {
                        b'f' => b'F',
                        b'F' => b'f',
                        b't' => b'T',
                        b'T' => b't',
                        _ => cmd,
                    };
                    for _ in 0..count {
                        if let Some(pos) = do_find(&self.line, self.cursor, rev, target) {
                            self.cursor = pos;
                        } else {
                            actions.push(ViAction::Bell);
                            break;
                        }
                    }
                    actions.push(ViAction::Redraw);
                }
            }
            b'x' => {
                self.last_cmd = Some((b'x', count, None));
                for _ in 0..count {
                    if self.cursor < self.line.len() {
                        self.yank_buf = vec![self.line.remove(self.cursor)];
                    } else {
                        break;
                    }
                    if self.cursor >= self.line.len() && self.cursor > 0 {
                        self.cursor -= 1;
                    }
                }
                actions.push(ViAction::Redraw);
            }
            b'X' => {
                self.last_cmd = Some((b'X', count, None));
                for _ in 0..count {
                    if self.cursor > 0 {
                        self.cursor -= 1;
                        self.yank_buf = vec![self.line.remove(self.cursor)];
                    } else {
                        actions.push(ViAction::Bell);
                        break;
                    }
                }
                actions.push(ViAction::Redraw);
            }
            b'r' => {
                self.pending = PendingInput::ReplaceChar { count };
                actions.push(ViAction::NeedReplaceChar);
            }
            b'R' => {
                self.pending = PendingInput::ReplaceMode;
                actions.push(ViAction::NeedReplaceModeInput);
            }
            b'~' => {
                for _ in 0..count {
                    if self.cursor < self.line.len() {
                        let c = self.line[self.cursor];
                        if c.is_ascii_lowercase() {
                            self.line[self.cursor] = c.to_ascii_uppercase();
                        } else if c.is_ascii_uppercase() {
                            self.line[self.cursor] = c.to_ascii_lowercase();
                        }
                        if self.cursor + 1 < self.line.len() {
                            self.cursor += 1;
                        } else {
                            break;
                        }
                    }
                }
                actions.push(ViAction::Redraw);
            }
            b'd' => {
                self.pending = PendingInput::Motion { op: b'd', count };
                actions.push(ViAction::NeedMotion);
            }
            b'D' => {
                if self.cursor < self.line.len() {
                    self.yank_buf = self.line[self.cursor..].to_vec();
                    self.line.truncate(self.cursor);
                    if self.cursor > 0 {
                        self.cursor -= 1;
                    }
                }
                actions.push(ViAction::Redraw);
            }
            b'c' => {
                self.pending = PendingInput::Motion { op: b'c', count };
                actions.push(ViAction::NeedMotion);
            }
            b'C' => {
                if self.cursor < self.line.len() {
                    self.yank_buf = self.line[self.cursor..].to_vec();
                    self.line.truncate(self.cursor);
                }
                self.insert_mode = true;
                actions.push(ViAction::Redraw);
                actions.push(ViAction::SetInsertMode(true));
            }
            b'S' => {
                self.yank_buf = self.line.clone();
                self.line.clear();
                self.cursor = 0;
                self.insert_mode = true;
                actions.push(ViAction::Redraw);
                actions.push(ViAction::SetInsertMode(true));
            }
            b'y' => {
                self.pending = PendingInput::Motion { op: b'y', count };
                actions.push(ViAction::NeedMotion);
            }
            b'Y' => {
                if self.cursor < self.line.len() {
                    self.yank_buf = self.line[self.cursor..].to_vec();
                }
            }
            b'p' => {
                if !self.yank_buf.is_empty() {
                    let pos = (self.cursor + 1).min(self.line.len());
                    for b in self.yank_buf.clone().iter().rev() {
                        self.line.insert(pos, *b);
                    }
                    self.cursor = pos + self.yank_buf.len() - 1;
                    actions.push(ViAction::Redraw);
                }
            }
            b'P' => {
                if !self.yank_buf.is_empty() {
                    let yb = self.yank_buf.clone();
                    for (i, b) in yb.iter().enumerate() {
                        self.line.insert(self.cursor + i, *b);
                    }
                    self.cursor += self.yank_buf.len().saturating_sub(1);
                    actions.push(ViAction::Redraw);
                }
            }
            b'u' => {
                let saved = self.line.clone();
                let saved_cursor = self.cursor;
                self.line.clear();
                self.line.extend_from_slice(&self.edit_line);
                self.edit_line = saved;
                self.cursor = saved_cursor.min(self.line.len().saturating_sub(1));
                actions.push(ViAction::Redraw);
            }
            b'U' => {
                if let Some(idx) = self.hist_index {
                    if idx < self.hist_len {
                        self.line = history[idx].to_vec();
                    }
                } else {
                    self.line.clear();
                }
                self.cursor = self.cursor.min(self.line.len().saturating_sub(1));
                if self.line.is_empty() {
                    self.cursor = 0;
                }
                actions.push(ViAction::Redraw);
            }
            b'.' => {
                if let Some((cmd, prev_count, arg)) = self.last_cmd {
                    let c = if first_byte.is_ascii_digit() && first_byte != b'0' {
                        count
                    } else {
                        prev_count
                    };
                    replay_cmd(
                        &mut self.line,
                        &mut self.cursor,
                        &mut self.yank_buf,
                        cmd,
                        c,
                        arg,
                    );
                    actions.push(ViAction::Redraw);
                }
            }
            b'k' | b'-' => {
                let hist_len = self.hist_len;
                let target = match self.hist_index {
                    None => {
                        if hist_len > 0 {
                            self.edit_line = self.line.clone();
                            Some(hist_len - 1)
                        } else {
                            None
                        }
                    }
                    Some(idx) => {
                        if idx > 0 {
                            Some(idx - 1)
                        } else {
                            None
                        }
                    }
                };
                if let Some(idx) = target {
                    self.hist_index = Some(idx);
                    self.line = history[idx].to_vec();
                    self.cursor = self.line.len().saturating_sub(1);
                    if self.line.is_empty() {
                        self.cursor = 0;
                    }
                    actions.push(ViAction::Redraw);
                } else {
                    actions.push(ViAction::Bell);
                }
            }
            b'j' | b'+' => {
                let hist_len = self.hist_len;
                if let Some(idx) = self.hist_index {
                    if idx + 1 < hist_len {
                        self.hist_index = Some(idx + 1);
                        self.line = history[idx + 1].to_vec();
                    } else {
                        self.hist_index = None;
                        self.line = self.edit_line.clone();
                    }
                    self.cursor = self.line.len().saturating_sub(1);
                    if self.line.is_empty() {
                        self.cursor = 0;
                    }
                    actions.push(ViAction::Redraw);
                } else {
                    actions.push(ViAction::Bell);
                }
            }
            b'G' => {
                let hist_len = self.hist_len;
                if first_byte.is_ascii_digit() && first_byte != b'0' {
                    let target = count.saturating_sub(1).min(hist_len.saturating_sub(1));
                    if target < hist_len {
                        if self.hist_index.is_none() {
                            self.edit_line = self.line.clone();
                        }
                        self.hist_index = Some(target);
                        self.line = history[target].to_vec();
                    }
                } else if hist_len > 0 {
                    if self.hist_index.is_none() {
                        self.edit_line = self.line.clone();
                    }
                    self.hist_index = Some(0);
                    self.line = history[0].to_vec();
                }
                self.cursor = self.line.len().saturating_sub(1);
                if self.line.is_empty() {
                    self.cursor = 0;
                }
                actions.push(ViAction::Redraw);
            }
            b'/' => {
                actions.push(ViAction::WriteBytes(b"/".to_vec()));
                self.search_buf.clear();
                self.pending = PendingInput::SearchInput { direction: b'/' };
                actions.push(ViAction::NeedSearchByte);
            }
            b'?' => {
                actions.push(ViAction::WriteBytes(b"?".to_vec()));
                self.search_buf.clear();
                self.pending = PendingInput::SearchInput { direction: b'?' };
                actions.push(ViAction::NeedSearchByte);
            }
            b'n' => {
                if !self.search_buf.is_empty() {
                    self.do_search(b'/', history, &mut actions);
                    actions.push(ViAction::Redraw);
                }
            }
            b'N' => {
                if !self.search_buf.is_empty() {
                    self.do_search(b'?', history, &mut actions);
                    actions.push(ViAction::Redraw);
                }
            }
            b'#' => {
                self.line.insert(0, b'#');
                let mut s = self.line.clone();
                s.push(b'\n');
                actions.push(ViAction::WriteBytes(b"\r\n".to_vec()));
                actions.push(ViAction::Return(Some(s)));
            }
            b'v' => {
                let mut tmp = b"/tmp/meiksh_vi_edit_".to_vec();
                bstr::push_u64(&mut tmp, sys::current_pid() as u64);
                if let Ok(fd) =
                    sys::open_file(&tmp, sys::O_WRONLY | sys::O_CREAT | sys::O_TRUNC, 0o600)
                {
                    let _ = sys::write_all_fd(fd, &self.line);
                    let _ = sys::write_all_fd(fd, b"\n");
                    let _ = sys::close_fd(fd);
                }
                actions.push(ViAction::RunEditor {
                    editor: Vec::new(),
                    tmp_path: tmp,
                });
            }
            b'*' => {
                let word_start = {
                    let mut p = self.cursor;
                    while p > 0 && !self.line[p - 1].is_ascii_whitespace() {
                        p -= 1;
                    }
                    p
                };
                let word_end_pos = {
                    let mut p = self.cursor;
                    while p < self.line.len() && !self.line[p].is_ascii_whitespace() {
                        p += 1;
                    }
                    p
                };
                let raw = &self.line[word_start..word_end_pos];
                let pattern = if raw.contains(&b'*') || raw.contains(&b'?') || raw.contains(&b'[') {
                    raw.to_vec()
                } else {
                    let mut p = raw.to_vec();
                    p.push(b'*');
                    p
                };
                if let Ok(expanded) = glob_expand(&pattern) {
                    let mut replacement = Vec::new();
                    for (i, entry) in expanded.iter().enumerate() {
                        if i > 0 {
                            replacement.push(b' ');
                        }
                        replacement.extend_from_slice(entry);
                    }
                    self.line.drain(word_start..word_end_pos);
                    for (i, b) in replacement.iter().enumerate() {
                        self.line.insert(word_start + i, *b);
                    }
                    self.cursor = word_start + replacement.len();
                    if self.cursor > 0 {
                        self.cursor -= 1;
                    }
                }
                actions.push(ViAction::Redraw);
            }
            b'\\' => {
                let word_start = {
                    let mut p = self.cursor;
                    while p > 0 && !self.line[p - 1].is_ascii_whitespace() {
                        p -= 1;
                    }
                    p
                };
                let word_end_pos = {
                    let mut p = self.cursor;
                    while p < self.line.len() && !self.line[p].is_ascii_whitespace() {
                        p += 1;
                    }
                    p
                };
                let prefix = self.line[word_start..word_end_pos].to_vec();
                let mut glob_pat = prefix.clone();
                glob_pat.push(b'*');
                if let Ok(matches) = glob_expand(&glob_pat) {
                    if matches.len() == 1 {
                        let replacement = &matches[0];
                        let is_dir = sys::stat_path(replacement)
                            .map(|s| s.is_dir())
                            .unwrap_or(false);
                        let mut rep = replacement.clone();
                        if is_dir {
                            rep.push(b'/');
                        }
                        self.line.drain(word_start..word_end_pos);
                        for (i, b) in rep.iter().enumerate() {
                            self.line.insert(word_start + i, *b);
                        }
                        self.cursor = word_start + rep.len();
                        if self.cursor > 0 && !is_dir {
                            self.cursor -= 1;
                        }
                    } else {
                        actions.push(ViAction::Bell);
                    }
                }
                actions.push(ViAction::Redraw);
            }
            0x03 => {
                actions.push(ViAction::WriteBytes(b"\r\n".to_vec()));
                actions.push(ViAction::Return(Some(Vec::new())));
            }
            b'\r' | b'\n' => {
                let mut s = self.line.clone();
                s.push(b'\n');
                actions.push(ViAction::WriteBytes(b"\r\n".to_vec()));
                actions.push(ViAction::Return(Some(s)));
            }
            _ => {
                actions.push(ViAction::Bell);
            }
        }
        actions
    }

    fn process_motion(
        &mut self,
        op: u8,
        motion: u8,
        count: usize,
        actions: &mut Vec<ViAction>,
    ) -> Vec<ViAction> {
        match op {
            b'd' => {
                if motion == b'd' {
                    self.yank_buf = self.line.clone();
                    self.line.clear();
                    self.cursor = 0;
                    self.last_cmd = Some((b'd', count, Some(b'd')));
                } else {
                    let (start, end) = resolve_motion(&self.line, self.cursor, motion, count);
                    if start != end {
                        self.yank_buf = self.line[start..end].to_vec();
                        self.line.drain(start..end);
                        self.cursor = start.min(self.line.len().saturating_sub(1));
                        self.last_cmd = Some((b'd', count, Some(motion)));
                    } else {
                        actions.push(ViAction::Bell);
                    }
                }
                actions.push(ViAction::Redraw);
            }
            b'c' => {
                if motion == b'c' {
                    self.yank_buf = self.line.clone();
                    self.line.clear();
                    self.cursor = 0;
                    self.last_cmd = Some((b'c', count, Some(b'c')));
                } else {
                    let (start, end) = resolve_motion(&self.line, self.cursor, motion, count);
                    if start != end {
                        self.yank_buf = self.line[start..end].to_vec();
                        self.line.drain(start..end);
                        self.cursor = start;
                        self.last_cmd = Some((b'c', count, Some(motion)));
                    }
                }
                self.insert_mode = true;
                actions.push(ViAction::Redraw);
                actions.push(ViAction::SetInsertMode(true));
            }
            b'y' => {
                if motion == b'y' {
                    self.yank_buf = self.line.clone();
                } else {
                    let (start, end) = resolve_motion(&self.line, self.cursor, motion, count);
                    if start != end {
                        self.yank_buf = self.line[start..end].to_vec();
                    }
                }
            }
            _ => {}
        }
        std::mem::take(actions)
    }

    pub(crate) fn do_search(
        &mut self,
        direction: u8,
        history: &[Box<[u8]>],
        actions: &mut Vec<ViAction>,
    ) {
        let pat = &self.search_buf;
        let hist_len = self.hist_len;
        match direction {
            b'/' => {
                let start = self
                    .hist_index
                    .map(|i| i.wrapping_sub(1))
                    .unwrap_or(hist_len.wrapping_sub(1));
                let mut found = false;
                let mut idx = start;
                for _ in 0..hist_len {
                    if idx >= hist_len {
                        break;
                    }
                    if history[idx].windows(pat.len()).any(|w| w == pat.as_slice()) {
                        self.hist_index = Some(idx);
                        self.line = history[idx].to_vec();
                        self.cursor = self.line.len().saturating_sub(1);
                        found = true;
                        break;
                    }
                    idx = idx.wrapping_sub(1);
                }
                if !found {
                    actions.push(ViAction::Bell);
                }
            }
            b'?' => {
                let start = self.hist_index.map(|i| (i + 1).min(hist_len)).unwrap_or(0);
                let mut found = false;
                for idx in start..hist_len {
                    if history[idx].windows(pat.len()).any(|w| w == pat.as_slice()) {
                        self.hist_index = Some(idx);
                        self.line = history[idx].to_vec();
                        self.cursor = self.line.len().saturating_sub(1);
                        found = true;
                        break;
                    }
                }
                if !found {
                    actions.push(ViAction::Bell);
                }
            }
            _ => {}
        }
    }
}

pub fn read_line(shell: &mut Shell, prompt: &[u8]) -> sys::SysResult<Option<Vec<u8>>> {
    let _raw = match RawMode::enter() {
        Ok(r) => r,
        Err(_) => return super::read_line(),
    };

    let erase_char = {
        if let Ok(attrs) = sys::get_terminal_attrs(sys::STDIN_FILENO) {
            attrs.c_cc[libc::VERASE]
        } else {
            0x7f
        }
    };

    let hist_len = shell.history.len();
    let mut state = ViState::new(erase_char, hist_len);

    loop {
        let byte = match read_byte()? {
            Some(b) => b,
            None => {
                if state.line.is_empty() && state.cursor == 0 {
                    write_bytes(b"\r\n");
                    return Ok(None);
                }
                continue;
            }
        };

        let actions = state.process_byte(byte, &shell.history);
        for action in actions {
            match action {
                ViAction::Redraw => {
                    redraw(&state.line, state.cursor, prompt);
                }
                ViAction::Bell => {
                    bell();
                }
                ViAction::Return(result) => {
                    return Ok(result);
                }
                ViAction::ReadByte => {}
                ViAction::WriteBytes(data) => {
                    write_bytes(&data);
                }
                ViAction::RunEditor { tmp_path, .. } => {
                    let editor = shell
                        .get_var(b"VISUAL")
                        .or_else(|| shell.get_var(b"EDITOR"))
                        .unwrap_or(b"vi")
                        .to_vec();
                    let _ = sys::set_terminal_attrs(sys::STDIN_FILENO, &_raw.saved);
                    write_bytes(b"\r\n");
                    let mut edit_cmd = editor;
                    edit_cmd.push(b' ');
                    edit_cmd.extend_from_slice(&tmp_path);
                    let _ = shell.execute_string(&edit_cmd);
                    let mut raw_restored = _raw.saved;
                    raw_restored.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG);
                    raw_restored.c_cc[libc::VMIN] = 1;
                    raw_restored.c_cc[libc::VTIME] = 0;
                    let _ = sys::set_terminal_attrs(sys::STDIN_FILENO, &raw_restored);
                    if let Ok(content) = sys::read_file(&tmp_path) {
                        let mut end = content.len();
                        while end > 0
                            && (content[end - 1] == b' '
                                || content[end - 1] == b'\t'
                                || content[end - 1] == b'\n'
                                || content[end - 1] == b'\r')
                        {
                            end -= 1;
                        }
                        let trimmed = &content[..end];
                        if !trimmed.is_empty() {
                            super::remove_file_bytes(&tmp_path);
                            write_bytes(b"\r\n");
                            let mut s = trimmed.to_vec();
                            s.push(b'\n');
                            return Ok(Some(s));
                        }
                    }
                    super::remove_file_bytes(&tmp_path);
                    redraw(&state.line, state.cursor, prompt);
                }
                ViAction::NeedSearchByte
                | ViAction::NeedFindTarget
                | ViAction::NeedReplaceChar
                | ViAction::NeedMotion
                | ViAction::NeedReplaceModeInput
                | ViAction::NeedLiteralChar => {}
                ViAction::SetInsertMode(_) => {}
            }
        }
    }
}

pub(crate) fn do_find(line: &[u8], cursor: usize, cmd: u8, target: u8) -> Option<usize> {
    match cmd {
        b'f' => {
            for i in (cursor + 1)..line.len() {
                if line[i] == target {
                    return Some(i);
                }
            }
            None
        }
        b'F' => {
            for i in (0..cursor).rev() {
                if line[i] == target {
                    return Some(i);
                }
            }
            None
        }
        b't' => {
            for i in (cursor + 1)..line.len() {
                if line[i] == target {
                    return if i > 0 { Some(i - 1) } else { None };
                }
            }
            None
        }
        b'T' => {
            for i in (0..cursor).rev() {
                if line[i] == target {
                    return Some(i + 1);
                }
            }
            None
        }
        _ => None,
    }
}

pub(crate) fn resolve_motion(
    line: &[u8],
    cursor: usize,
    motion: u8,
    count: usize,
) -> (usize, usize) {
    let target = match motion {
        b'w' => {
            let mut p = cursor;
            for _ in 0..count {
                p = word_forward(line, p);
            }
            p
        }
        b'W' => {
            let mut p = cursor;
            for _ in 0..count {
                p = bigword_forward(line, p);
            }
            p
        }
        b'b' => {
            let mut p = cursor;
            for _ in 0..count {
                p = word_backward(line, p);
            }
            p
        }
        b'B' => {
            let mut p = cursor;
            for _ in 0..count {
                p = bigword_backward(line, p);
            }
            p
        }
        b'e' => {
            let mut p = cursor;
            for _ in 0..count {
                p = word_end(line, p);
            }
            p + 1
        }
        b'E' => {
            let mut p = cursor;
            for _ in 0..count {
                p = bigword_end(line, p);
            }
            p + 1
        }
        b'h' => return (cursor.saturating_sub(count), cursor),
        b'l' | b' ' => {
            let end = (cursor + count).min(line.len());
            return (cursor, end);
        }
        b'0' => return (0, cursor),
        b'$' => return (cursor, line.len()),
        _ => return (cursor, cursor),
    };
    if target < cursor {
        (target, cursor)
    } else {
        (cursor, target.min(line.len()))
    }
}

pub(crate) fn replay_cmd(
    line: &mut Vec<u8>,
    cursor: &mut usize,
    yank_buf: &mut Vec<u8>,
    cmd: u8,
    count: usize,
    arg: Option<u8>,
) {
    match cmd {
        b'x' => {
            for _ in 0..count {
                if *cursor < line.len() {
                    *yank_buf = vec![line.remove(*cursor)];
                }
                if *cursor >= line.len() && *cursor > 0 {
                    *cursor -= 1;
                }
            }
        }
        b'X' => {
            for _ in 0..count {
                if *cursor > 0 {
                    *cursor -= 1;
                    *yank_buf = vec![line.remove(*cursor)];
                }
            }
        }
        b'r' => {
            if let Some(replacement) = arg {
                for _ in 0..count {
                    if *cursor < line.len() {
                        line[*cursor] = replacement;
                        if *cursor + 1 < line.len() {
                            *cursor += 1;
                        }
                    }
                }
                if count > 1 && *cursor > 0 {
                    *cursor -= 1;
                }
            }
        }
        b'd' => {
            if let Some(motion) = arg {
                if motion == b'd' {
                    *yank_buf = line.clone();
                    line.clear();
                    *cursor = 0;
                } else {
                    let (start, end) = resolve_motion(line, *cursor, motion, count);
                    if start != end {
                        *yank_buf = line[start..end].to_vec();
                        line.drain(start..end);
                        *cursor = start.min(line.len().saturating_sub(1));
                    }
                }
            }
        }
        b'c' => {
            if let Some(motion) = arg {
                if motion == b'c' {
                    *yank_buf = line.clone();
                    line.clear();
                    *cursor = 0;
                } else {
                    let (start, end) = resolve_motion(line, *cursor, motion, count);
                    if start != end {
                        *yank_buf = line[start..end].to_vec();
                        line.drain(start..end);
                        *cursor = start;
                    }
                }
            }
        }
        _ => {}
    }
}

pub(crate) fn glob_expand(pattern: &[u8]) -> Result<Vec<Vec<u8>>, ()> {
    let c_pattern = std::ffi::CString::new(pattern.to_vec()).map_err(|_| ())?;
    let mut glob_buf: libc::glob_t = unsafe { std::mem::zeroed() };
    let ret = unsafe {
        libc::glob(
            c_pattern.as_ptr(),
            libc::GLOB_TILDE | libc::GLOB_MARK,
            None,
            &mut glob_buf,
        )
    };
    if ret != 0 {
        unsafe { libc::globfree(&mut glob_buf) };
        return Err(());
    }
    let mut results = Vec::new();
    for i in 0..glob_buf.gl_pathc {
        let path = unsafe { std::ffi::CStr::from_ptr(*glob_buf.gl_pathv.add(i)) };
        results.push(path.to_bytes().to_vec());
    }
    unsafe { libc::globfree(&mut glob_buf) };
    Ok(results)
}
