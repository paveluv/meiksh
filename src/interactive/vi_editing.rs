use crate::bstr::{self, ByteWriter};
use crate::shell::state::Shell;
use crate::sys;

struct RawMode {
    saved: libc::termios,
}

impl RawMode {
    fn enter() -> sys::error::SysResult<Self> {
        let saved = sys::tty::get_terminal_attrs(sys::constants::STDIN_FILENO)?;
        let mut raw = saved;
        raw.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG);
        raw.c_cc[libc::VMIN] = 1;
        raw.c_cc[libc::VTIME] = 0;
        sys::tty::set_terminal_attrs(sys::constants::STDIN_FILENO, &raw)?;
        Ok(Self { saved })
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = sys::tty::set_terminal_attrs(sys::constants::STDIN_FILENO, &self.saved);
    }
}

fn read_byte() -> sys::error::SysResult<Option<u8>> {
    let mut buf = [0u8; 1];
    match sys::fd_io::read_fd(sys::constants::STDIN_FILENO, &mut buf) {
        Ok(0) => Ok(None),
        Ok(_) => Ok(Some(buf[0])),
        Err(e) => Err(e),
    }
}

fn write_bytes(data: &[u8]) {
    let _ = sys::fd_io::write_all_fd(sys::constants::STDOUT_FILENO, data);
}

fn bell() {
    write_bytes(b"\x07");
}

fn display_width(line: &[u8]) -> usize {
    let mut w = 0;
    let mut i = 0;
    while i < line.len() {
        let (wc, len) = sys::locale::decode_char(&line[i..]);
        let step = if len == 0 { 1 } else { len };
        w += sys::locale::char_width(wc);
        i += step;
    }
    w
}

fn display_width_range(line: &[u8], from: usize, to: usize) -> usize {
    if to <= from {
        return 0;
    }
    display_width(&line[from..to])
}

fn char_len_at(line: &[u8], pos: usize) -> usize {
    if pos >= line.len() {
        return 0;
    }
    let (_, len) = sys::locale::decode_char(&line[pos..]);
    if len == 0 { 1 } else { len }
}

fn prev_char_start(line: &[u8], pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    let mut p = pos - 1;
    while p > 0 && (line[p] & 0xC0) == 0x80 {
        p -= 1;
    }
    p
}

fn redraw(line: &[u8], cursor: usize, prompt: &[u8]) {
    write_bytes(b"\r\x1b[K");
    let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, prompt);
    let mut buf = Vec::with_capacity(line.len() + 20);
    buf.extend_from_slice(line);
    let cursor_back = display_width_range(line, cursor, line.len());
    if cursor_back > 0 {
        buf.extend_from_slice(b"\x1b[");
        bstr::push_u64(&mut buf, cursor_back as u64);
        buf.push(b'D');
    }
    write_bytes(&buf);
}

fn is_word_char_wc(wc: u32) -> bool {
    if wc == b'_' as u32 {
        return true;
    }
    sys::locale::classify_char(b"alnum", wc)
}

#[allow(dead_code)]
fn is_word_char(c: u8) -> bool {
    is_word_char_wc(c as u32)
}

fn expected_utf8_len(first_byte: u8) -> usize {
    if first_byte < 0x80 {
        1
    } else if first_byte < 0xC0 {
        1
    } else if first_byte < 0xE0 {
        2
    } else if first_byte < 0xF0 {
        3
    } else {
        4
    }
}

fn last_char_start(line: &[u8]) -> usize {
    if line.is_empty() {
        return 0;
    }
    prev_char_start(line, line.len())
}

fn is_word_char_at(line: &[u8], pos: usize) -> bool {
    let (wc, _) = sys::locale::decode_char(&line[pos..]);
    is_word_char_wc(wc)
}

fn is_ws_at(line: &[u8], pos: usize) -> bool {
    let b = line[pos];
    b == b' ' || b == b'\t' || b == b'\n'
}

fn is_word_char_before(line: &[u8], pos: usize) -> bool {
    is_word_char_at(line, prev_char_start(line, pos))
}

fn is_ws_before(line: &[u8], pos: usize) -> bool {
    is_ws_at(line, prev_char_start(line, pos))
}

fn word_forward(line: &[u8], pos: usize) -> usize {
    let mut p = pos;
    let len = line.len();
    if p >= len {
        return p;
    }
    if is_word_char_at(line, p) {
        while p < len && is_word_char_at(line, p) {
            p += char_len_at(line, p);
        }
    } else if !is_ws_at(line, p) {
        while p < len && !is_word_char_at(line, p) && !is_ws_at(line, p) {
            p += char_len_at(line, p);
        }
    }
    while p < len && is_ws_at(line, p) {
        p += char_len_at(line, p);
    }
    p
}

fn word_backward(line: &[u8], pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    let mut p = pos;
    while p > 0 && is_ws_before(line, p) {
        p = prev_char_start(line, p);
    }
    if p == 0 {
        return 0;
    }
    if is_word_char_before(line, p) {
        while p > 0 && is_word_char_before(line, p) {
            p = prev_char_start(line, p);
        }
    } else {
        while p > 0 && !is_word_char_before(line, p) && !is_ws_before(line, p) {
            p = prev_char_start(line, p);
        }
    }
    p
}

fn bigword_forward(line: &[u8], pos: usize) -> usize {
    let mut p = pos;
    let len = line.len();
    while p < len && !is_ws_at(line, p) {
        p += char_len_at(line, p);
    }
    while p < len && is_ws_at(line, p) {
        p += char_len_at(line, p);
    }
    p
}

fn bigword_backward(line: &[u8], pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    let mut p = pos;
    while p > 0 && is_ws_before(line, p) {
        p = prev_char_start(line, p);
    }
    while p > 0 && !is_ws_before(line, p) {
        p = prev_char_start(line, p);
    }
    p
}

fn word_end(line: &[u8], pos: usize) -> usize {
    let len = line.len();
    let next = pos + char_len_at(line, pos);
    if next >= len {
        return pos;
    }
    let mut p = next;
    while p < len && is_ws_at(line, p) {
        p += char_len_at(line, p);
    }
    if p >= len {
        return last_char_start(line);
    }
    if is_word_char_at(line, p) {
        loop {
            let n = p + char_len_at(line, p);
            if n >= len || !is_word_char_at(line, n) {
                break;
            }
            p = n;
        }
    } else {
        loop {
            let n = p + char_len_at(line, p);
            if n >= len || is_word_char_at(line, n) || is_ws_at(line, n) {
                break;
            }
            p = n;
        }
    }
    p
}

fn bigword_end(line: &[u8], pos: usize) -> usize {
    let len = line.len();
    let next = pos + char_len_at(line, pos);
    if next >= len {
        return pos;
    }
    let mut p = next;
    while p < len && is_ws_at(line, p) {
        p += char_len_at(line, p);
    }
    if p >= len {
        return last_char_start(line);
    }
    loop {
        let n = p + char_len_at(line, p);
        if n >= len || is_ws_at(line, n) {
            break;
        }
        p = n;
    }
    p
}

#[derive(Clone, Debug, PartialEq)]
enum ViAction {
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
enum PendingInput {
    None,
    CountDigits,
    FindTarget { cmd: u8, count: usize, buf: Vec<u8> },
    ReplaceChar { count: usize, buf: Vec<u8> },
    ReplaceMode,
    Motion { op: u8, count: usize },
    LiteralChar,
    SearchInput { direction: u8 },
}

struct ViState {
    pub line: Vec<u8>,
    pub cursor: usize,
    pub insert_mode: bool,
    pub yank_buf: Vec<u8>,
    pub last_cmd: Option<(u8, usize, Option<u8>)>,
    pub last_find: Option<(u8, u32)>,
    pub hist_index: Option<usize>,
    pub edit_line: Vec<u8>,
    pub search_buf: Vec<u8>,
    pub count_buf: Option<(usize, u8)>,
    pub pending: PendingInput,
    erase_char: u8,
    hist_len: usize,
}

impl ViState {
    fn new(erase_char: u8, hist_len: usize) -> Self {
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

    fn process_byte(&mut self, byte: u8, history: &[Box<[u8]>]) -> Vec<ViAction> {
        let mut actions = Vec::new();

        match &mut self.pending {
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
            PendingInput::FindTarget { cmd, count, buf } => {
                let cmd = *cmd;
                let count = *count;
                buf.push(byte);
                let expected_len = expected_utf8_len(buf[0]);
                if buf.len() < expected_len {
                    return vec![ViAction::ReadByte];
                }
                let (wc, len) = sys::locale::decode_char(buf);
                let target = if len > 0 { wc } else { buf[0] as u32 };
                self.pending = PendingInput::None;
                self.last_find = Some((cmd, target));
                for _ in 0..count {
                    if let Some(pos) = do_find(&self.line, self.cursor, cmd, target) {
                        self.cursor = pos;
                    } else {
                        actions.push(ViAction::Bell);
                        break;
                    }
                }
                actions.push(ViAction::Redraw);
                return actions;
            }
            PendingInput::ReplaceChar { count, buf } => {
                let count = *count;
                buf.push(byte);
                let expected_len = expected_utf8_len(buf[0]);
                if buf.len() < expected_len {
                    return vec![ViAction::ReadByte];
                }
                let (_, len) = sys::locale::decode_char(buf);
                let replacement: Vec<u8> = if len > 0 {
                    buf[..len].to_vec()
                } else {
                    vec![buf[0]]
                };
                self.pending = PendingInput::None;
                self.last_cmd = Some((b'r', count, Some(replacement[0])));
                for _ in 0..count {
                    if self.cursor < self.line.len() {
                        let clen = char_len_at(&self.line, self.cursor);
                        self.line.drain(self.cursor..self.cursor + clen);
                        for (j, &rb) in replacement.iter().enumerate() {
                            self.line.insert(self.cursor + j, rb);
                        }
                        let next = self.cursor + replacement.len();
                        if next < self.line.len() {
                            self.cursor = next;
                        }
                    }
                }
                if count > 1 && self.cursor > 0 {
                    self.cursor = prev_char_start(&self.line, self.cursor);
                }
                actions.push(ViAction::Redraw);
                return actions;
            }
            PendingInput::ReplaceMode => match byte {
                0x1b => {
                    self.pending = PendingInput::None;
                    if self.cursor > 0 && self.cursor >= self.line.len() {
                        self.cursor = last_char_start(&self.line);
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
                        let clen = char_len_at(&self.line, self.cursor);
                        self.line.drain(self.cursor..self.cursor + clen);
                        self.line.insert(self.cursor, b);
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
                            let last = prev_char_start(&self.search_buf, self.search_buf.len());
                            self.search_buf.truncate(last);
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
                        self.cursor = last_char_start(&self.line);
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
                        let prev = prev_char_start(&self.line, self.cursor);
                        self.line.drain(prev..self.cursor);
                        self.cursor = prev;
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
                    self.cursor =
                        (self.cursor + char_len_at(&self.line, self.cursor)).min(self.line.len());
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
                let old = self.cursor;
                for _ in 0..count {
                    if self.cursor == 0 {
                        break;
                    }
                    self.cursor = prev_char_start(&self.line, self.cursor);
                }
                if self.cursor != old {
                    let cols = display_width_range(&self.line, self.cursor, old);
                    let esc = ByteWriter::new()
                        .bytes(b"\x1b[")
                        .usize_val(cols)
                        .byte(b'D')
                        .finish();
                    actions.push(ViAction::WriteBytes(esc));
                } else {
                    actions.push(ViAction::Bell);
                }
            }
            b'l' | b' ' => {
                let old = self.cursor;
                for _ in 0..count {
                    let clen = char_len_at(&self.line, self.cursor);
                    if self.cursor + clen >= self.line.len() {
                        break;
                    }
                    self.cursor += clen;
                }
                if self.cursor != old {
                    let cols = display_width_range(&self.line, old, self.cursor);
                    let esc = ByteWriter::new()
                        .bytes(b"\x1b[")
                        .usize_val(cols)
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
                    self.cursor = last_char_start(&self.line);
                }
                actions.push(ViAction::Redraw);
            }
            b'^' => {
                let mut p = 0;
                while p < self.line.len() && is_ws_at(&self.line, p) {
                    p += char_len_at(&self.line, p);
                }
                self.cursor = if p < self.line.len() { p } else { 0 };
                actions.push(ViAction::Redraw);
            }
            b'w' => {
                for _ in 0..count {
                    let next = word_forward(&self.line, self.cursor);
                    if next == self.cursor {
                        actions.push(ViAction::Bell);
                        break;
                    }
                    self.cursor = if self.line.is_empty() {
                        0
                    } else {
                        next.min(last_char_start(&self.line))
                    };
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
                    self.cursor = if self.line.is_empty() {
                        0
                    } else {
                        next.min(last_char_start(&self.line))
                    };
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
                let target_col = count.saturating_sub(1);
                let mut p = 0;
                let mut col = 0;
                while p < self.line.len() && col < target_col {
                    p += char_len_at(&self.line, p);
                    col += 1;
                }
                self.cursor = if p > self.line.len() {
                    last_char_start(&self.line)
                } else {
                    p.min(if self.line.is_empty() {
                        0
                    } else {
                        last_char_start(&self.line)
                    })
                };
                actions.push(ViAction::Redraw);
            }
            b'f' | b'F' | b't' | b'T' => {
                self.pending = PendingInput::FindTarget {
                    cmd: ch,
                    count,
                    buf: Vec::new(),
                };
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
                        let clen = char_len_at(&self.line, self.cursor);
                        self.yank_buf = self.line[self.cursor..self.cursor + clen].to_vec();
                        self.line.drain(self.cursor..self.cursor + clen);
                    } else {
                        break;
                    }
                    if self.cursor >= self.line.len() && self.cursor > 0 {
                        self.cursor = prev_char_start(&self.line, self.cursor);
                    }
                }
                actions.push(ViAction::Redraw);
            }
            b'X' => {
                self.last_cmd = Some((b'X', count, None));
                for _ in 0..count {
                    if self.cursor > 0 {
                        let prev = prev_char_start(&self.line, self.cursor);
                        self.yank_buf = self.line[prev..self.cursor].to_vec();
                        self.line.drain(prev..self.cursor);
                        self.cursor = prev;
                    } else {
                        actions.push(ViAction::Bell);
                        break;
                    }
                }
                actions.push(ViAction::Redraw);
            }
            b'r' => {
                self.pending = PendingInput::ReplaceChar {
                    count,
                    buf: Vec::new(),
                };
                actions.push(ViAction::NeedReplaceChar);
            }
            b'R' => {
                self.pending = PendingInput::ReplaceMode;
                actions.push(ViAction::NeedReplaceModeInput);
            }
            b'~' => {
                for _ in 0..count {
                    if self.cursor < self.line.len() {
                        let clen = char_len_at(&self.line, self.cursor);
                        let (wc, _) = sys::locale::decode_char(&self.line[self.cursor..]);
                        let toggled = if sys::locale::classify_char(b"lower", wc) {
                            sys::locale::to_upper(wc)
                        } else if sys::locale::classify_char(b"upper", wc) {
                            sys::locale::to_lower(wc)
                        } else {
                            wc
                        };
                        if toggled != wc {
                            let encoded = sys::locale::encode_char(toggled);
                            self.line
                                .splice(self.cursor..self.cursor + clen, encoded.iter().copied());
                            let new_clen = char_len_at(&self.line, self.cursor);
                            if self.cursor + new_clen < self.line.len() {
                                self.cursor += new_clen;
                            } else {
                                break;
                            }
                        } else {
                            if self.cursor + clen < self.line.len() {
                                self.cursor += clen;
                            } else {
                                break;
                            }
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
                        self.cursor = last_char_start(&self.line);
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
                    let pos = if self.line.is_empty() {
                        0
                    } else {
                        (self.cursor + char_len_at(&self.line, self.cursor)).min(self.line.len())
                    };
                    let yb = self.yank_buf.clone();
                    for b in yb.iter().rev() {
                        self.line.insert(pos, *b);
                    }
                    let pasted_end = pos + yb.len();
                    self.cursor = last_char_start(&self.line[..pasted_end]);
                    actions.push(ViAction::Redraw);
                }
            }
            b'P' => {
                if !self.yank_buf.is_empty() {
                    let yb = self.yank_buf.clone();
                    for (i, b) in yb.iter().enumerate() {
                        self.line.insert(self.cursor + i, *b);
                    }
                    let pasted_end = self.cursor + yb.len();
                    self.cursor = last_char_start(&self.line[..pasted_end]);
                    actions.push(ViAction::Redraw);
                }
            }
            b'u' => {
                let saved = self.line.clone();
                let saved_cursor = self.cursor;
                self.line.clear();
                self.line.extend_from_slice(&self.edit_line);
                self.edit_line = saved;
                self.cursor = if self.line.is_empty() {
                    0
                } else {
                    saved_cursor.min(last_char_start(&self.line))
                };
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
                self.cursor = if self.line.is_empty() {
                    0
                } else {
                    self.cursor.min(last_char_start(&self.line))
                };
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
                    self.cursor = if self.line.is_empty() {
                        0
                    } else {
                        last_char_start(&self.line)
                    };
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
                    self.cursor = if self.line.is_empty() {
                        0
                    } else {
                        last_char_start(&self.line)
                    };
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
                self.cursor = if self.line.is_empty() {
                    0
                } else {
                    last_char_start(&self.line)
                };
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
                bstr::push_u64(&mut tmp, sys::process::current_pid() as u64);
                if let Ok(fd) = sys::fs::open_file(
                    &tmp,
                    sys::constants::O_WRONLY | sys::constants::O_CREAT | sys::constants::O_TRUNC,
                    0o600,
                ) {
                    let _ = sys::fd_io::write_all_fd(fd, &self.line);
                    let _ = sys::fd_io::write_all_fd(fd, b"\n");
                    let _ = sys::fd_io::close_fd(fd);
                }
                actions.push(ViAction::RunEditor {
                    editor: Vec::new(),
                    tmp_path: tmp,
                });
            }
            b'*' => {
                let word_start = {
                    let mut p = self.cursor;
                    while p > 0 && !is_ws_before(&self.line, p) {
                        p = prev_char_start(&self.line, p);
                    }
                    p
                };
                let word_end_pos = {
                    let mut p = self.cursor;
                    while p < self.line.len() && !is_ws_at(&self.line, p) {
                        p += char_len_at(&self.line, p);
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
                    let end = word_start + replacement.len();
                    self.cursor = if end > 0 {
                        last_char_start(&self.line[..end])
                    } else {
                        0
                    };
                }
                actions.push(ViAction::Redraw);
            }
            b'\\' => {
                let word_start = {
                    let mut p = self.cursor;
                    while p > 0 && !is_ws_before(&self.line, p) {
                        p = prev_char_start(&self.line, p);
                    }
                    p
                };
                let word_end_pos = {
                    let mut p = self.cursor;
                    while p < self.line.len() && !is_ws_at(&self.line, p) {
                        p += char_len_at(&self.line, p);
                    }
                    p
                };
                let prefix = self.line[word_start..word_end_pos].to_vec();
                let mut glob_pat = prefix.clone();
                glob_pat.push(b'*');
                if let Ok(matches) = glob_expand(&glob_pat) {
                    if matches.len() == 1 {
                        let replacement = &matches[0];
                        let is_dir = sys::fs::stat_path(replacement)
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
                        let end = word_start + rep.len();
                        if is_dir {
                            self.cursor = end;
                        } else {
                            self.cursor = if end > 0 {
                                last_char_start(&self.line[..end])
                            } else {
                                0
                            };
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
                        self.cursor = if self.line.is_empty() {
                            0
                        } else {
                            start.min(last_char_start(&self.line))
                        };
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

    fn do_search(&mut self, direction: u8, history: &[Box<[u8]>], actions: &mut Vec<ViAction>) {
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
                        self.cursor = if self.line.is_empty() {
                            0
                        } else {
                            last_char_start(&self.line)
                        };
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
                        self.cursor = if self.line.is_empty() {
                            0
                        } else {
                            last_char_start(&self.line)
                        };
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

pub(super) fn read_line(
    shell: &mut Shell,
    prompt: &[u8],
) -> sys::error::SysResult<Option<Vec<u8>>> {
    let _raw = match RawMode::enter() {
        Ok(r) => r,
        Err(_) => return super::prompt::read_line(),
    };

    let erase_char = {
        if let Ok(attrs) = sys::tty::get_terminal_attrs(sys::constants::STDIN_FILENO) {
            attrs.c_cc[libc::VERASE]
        } else {
            0x7f
        }
    };

    let hist_len = shell.history().len();
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

        let actions = state.process_byte(byte, &shell.history());
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
                    let _ = sys::tty::set_terminal_attrs(sys::constants::STDIN_FILENO, &_raw.saved);
                    write_bytes(b"\r\n");
                    let mut edit_cmd = editor;
                    edit_cmd.push(b' ');
                    edit_cmd.extend_from_slice(&tmp_path);
                    let _ = shell.execute_string(&edit_cmd);
                    let mut raw_restored = _raw.saved;
                    raw_restored.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG);
                    raw_restored.c_cc[libc::VMIN] = 1;
                    raw_restored.c_cc[libc::VTIME] = 0;
                    let _ =
                        sys::tty::set_terminal_attrs(sys::constants::STDIN_FILENO, &raw_restored);
                    if let Ok(content) = sys::fs::read_file(&tmp_path) {
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

fn do_find(line: &[u8], cursor: usize, cmd: u8, target: u32) -> Option<usize> {
    match cmd {
        b'f' => {
            let mut i = cursor + char_len_at(line, cursor);
            while i < line.len() {
                let (wc, len) = sys::locale::decode_char(&line[i..]);
                let step = if len == 0 { 1 } else { len };
                if wc == target {
                    return Some(i);
                }
                i += step;
            }
            None
        }
        b'F' => {
            let mut i = cursor;
            while i > 0 {
                i = prev_char_start(line, i);
                let (wc, _) = sys::locale::decode_char(&line[i..]);
                if wc == target {
                    return Some(i);
                }
            }
            None
        }
        b't' => {
            let mut i = cursor + char_len_at(line, cursor);
            while i < line.len() {
                let (wc, len) = sys::locale::decode_char(&line[i..]);
                let step = if len == 0 { 1 } else { len };
                if wc == target {
                    return Some(prev_char_start(line, i));
                }
                i += step;
            }
            None
        }
        b'T' => {
            let mut i = cursor;
            while i > 0 {
                i = prev_char_start(line, i);
                let (wc, _) = sys::locale::decode_char(&line[i..]);
                if wc == target {
                    return Some(i + char_len_at(line, i));
                }
            }
            None
        }
        _ => None,
    }
}

fn resolve_motion(line: &[u8], cursor: usize, motion: u8, count: usize) -> (usize, usize) {
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
            p + char_len_at(line, p)
        }
        b'E' => {
            let mut p = cursor;
            for _ in 0..count {
                p = bigword_end(line, p);
            }
            p + char_len_at(line, p)
        }
        b'h' => {
            let mut p = cursor;
            for _ in 0..count {
                if p == 0 {
                    break;
                }
                p = prev_char_start(line, p);
            }
            return (p, cursor);
        }
        b'l' | b' ' => {
            let mut p = cursor;
            for _ in 0..count {
                if p >= line.len() {
                    break;
                }
                p += char_len_at(line, p);
            }
            return (cursor, p.min(line.len()));
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

fn replay_cmd(
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
                    let clen = char_len_at(line, *cursor);
                    *yank_buf = line[*cursor..*cursor + clen].to_vec();
                    line.drain(*cursor..*cursor + clen);
                }
                if *cursor >= line.len() && *cursor > 0 {
                    *cursor = last_char_start(line);
                }
            }
        }
        b'X' => {
            for _ in 0..count {
                if *cursor > 0 {
                    let prev = prev_char_start(line, *cursor);
                    *yank_buf = line[prev..*cursor].to_vec();
                    line.drain(prev..*cursor);
                    *cursor = prev;
                }
            }
        }
        b'r' => {
            if let Some(replacement) = arg {
                for _ in 0..count {
                    if *cursor < line.len() {
                        let clen = char_len_at(line, *cursor);
                        line.drain(*cursor..*cursor + clen);
                        line.insert(*cursor, replacement);
                        let next = *cursor + 1;
                        if next < line.len() {
                            *cursor = next;
                        }
                    }
                }
                if count > 1 && *cursor > 0 {
                    *cursor = prev_char_start(line, *cursor);
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
                        *cursor = if line.is_empty() {
                            0
                        } else {
                            start.min(last_char_start(line))
                        };
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

#[cfg(target_os = "linux")]
const fn glob_tilde() -> libc::c_int {
    libc::GLOB_TILDE
}

#[cfg(not(target_os = "linux"))]
const fn glob_tilde() -> libc::c_int {
    0x0800
}

fn glob_expand(pattern: &[u8]) -> Result<Vec<Vec<u8>>, ()> {
    let c_pattern = std::ffi::CString::new(pattern.to_vec()).map_err(|_| ())?;
    let mut glob_buf: libc::glob_t = unsafe { std::mem::zeroed() };
    let ret = unsafe {
        libc::glob(
            c_pattern.as_ptr(),
            glob_tilde() | libc::GLOB_MARK,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interactive::test_support::test_shell;
    use crate::sys::constants::{STDIN_FILENO, STDOUT_FILENO};
    use crate::sys::test_support::run_trace;
    use crate::trace_entries;

    fn has_return(actions: &[ViAction]) -> bool {
        actions.iter().any(|a| matches!(a, ViAction::Return(_)))
    }

    fn get_return(actions: &[ViAction]) -> Option<Option<Vec<u8>>> {
        actions.iter().find_map(|a| match a {
            ViAction::Return(s) => Some(s.clone()),
            _ => None,
        })
    }

    fn has_bell(actions: &[ViAction]) -> bool {
        actions.iter().any(|a| matches!(a, ViAction::Bell))
    }

    fn feed_bytes(state: &mut ViState, bytes: &[u8], history: &[Box<[u8]>]) -> Vec<ViAction> {
        let mut all = Vec::new();
        for &b in bytes {
            all.extend(state.process_byte(b, history));
        }
        all
    }

    #[allow(non_snake_case)]
    mod vi_tests {
        use super::super::{
            PendingInput, ViAction, ViState, bigword_backward, bigword_end, bigword_forward,
            char_len_at, do_find, glob_expand, is_word_char, last_char_start, prev_char_start,
            replay_cmd, resolve_motion, word_backward, word_end, word_forward,
        };
        use super::{feed_bytes, get_return, has_bell, has_return};
        use crate::sys::test_support::{assert_no_syscalls, run_trace, set_test_locale_utf8};
        use crate::trace_entries;

        #[test]
        fn word_forward_covers_all_branches() {
            assert_no_syscalls(|| {
                assert_eq!(word_forward(b"hello world", 0), 6);
                assert_eq!(word_forward(b"hello world", 5), 6);
                assert_eq!(word_forward(b"hello", 5), 5);
                assert_eq!(word_forward(b"a.b cd", 1), 2);
                assert_eq!(word_forward(b"   a", 0), 3);
            });
        }

        #[test]
        fn word_backward_covers_all_branches() {
            assert_no_syscalls(|| {
                assert_eq!(word_backward(b"hello world", 6), 0);
                assert_eq!(word_backward(b"hello world", 11), 6);
                assert_eq!(word_backward(b"hello", 0), 0);
                assert_eq!(word_backward(b"a.b cd", 3), 2);
                assert_eq!(word_backward(b"  ab", 4), 2);
            });
        }

        #[test]
        fn bigword_forward_and_backward() {
            assert_no_syscalls(|| {
                assert_eq!(bigword_forward(b"a.b c.d", 0), 4);
                assert_eq!(bigword_forward(b"abc", 0), 3);
                assert_eq!(bigword_backward(b"a.b c.d", 4), 0);
                assert_eq!(bigword_backward(b"a.b c.d", 0), 0);
                assert_eq!(bigword_backward(b"ab   cd", 7), 5);
            });
        }

        #[test]
        fn word_end_and_bigword_end() {
            assert_no_syscalls(|| {
                assert_eq!(word_end(b"ab cd", 0), 1);
                assert_eq!(word_end(b"ab cd", 2), 4);
                assert_eq!(word_end(b"a", 0), 0);
                assert_eq!(word_end(b"a  b", 0), 3);
                assert_eq!(bigword_end(b"a.b c.d", 0), 2);
                assert_eq!(bigword_end(b"a", 0), 0);
                assert_eq!(bigword_end(b"a  b", 0), 3);
                assert_eq!(bigword_end(b"ab", 1), 1);
            });
        }

        #[test]
        fn is_word_char_tests() {
            assert_no_syscalls(|| {
                assert!(is_word_char(b'a'));
                assert!(is_word_char(b'Z'));
                assert!(is_word_char(b'0'));
                assert!(is_word_char(b'_'));
                assert!(!is_word_char(b'.'));
                assert!(!is_word_char(b' '));
            });
        }

        #[test]
        fn do_find_all_directions() {
            assert_no_syscalls(|| {
                let line = b"abcba";
                assert_eq!(do_find(line, 0, b'f', b'c' as u32), Some(2));
                assert_eq!(do_find(line, 0, b'f', b'z' as u32), None);
                assert_eq!(do_find(line, 4, b'F', b'c' as u32), Some(2));
                assert_eq!(do_find(line, 0, b'F', b'c' as u32), None);
                assert_eq!(do_find(line, 0, b't', b'c' as u32), Some(1));
                assert_eq!(do_find(line, 0, b't', b'z' as u32), None);
                assert_eq!(do_find(line, 4, b'T', b'c' as u32), Some(3));
                assert_eq!(do_find(line, 0, b'T', b'c' as u32), None);
                assert_eq!(do_find(line, 0, b'z', b'a' as u32), None);
            });
        }

        #[test]
        fn resolve_motion_covers_all_motions() {
            assert_no_syscalls(|| {
                let line = b"hello world";
                assert_eq!(resolve_motion(line, 0, b'w', 1), (0, 6));
                assert_eq!(resolve_motion(line, 6, b'b', 1), (0, 6));
                assert_eq!(resolve_motion(line, 0, b'W', 1), (0, 6));
                assert_eq!(resolve_motion(line, 6, b'B', 1), (0, 6));
                assert_eq!(resolve_motion(line, 0, b'e', 1), (0, 5));
                assert_eq!(resolve_motion(line, 0, b'E', 1), (0, 5));
                assert_eq!(resolve_motion(line, 5, b'h', 3), (2, 5));
                assert_eq!(resolve_motion(line, 2, b'l', 3), (2, 5));
                assert_eq!(resolve_motion(line, 5, b'0', 1), (0, 5));
                assert_eq!(resolve_motion(line, 0, b'$', 1), (0, 11));
                assert_eq!(resolve_motion(line, 0, b'z', 1), (0, 0));
            });
        }

        #[test]
        fn replay_cmd_x_and_X() {
            assert_no_syscalls(|| {
                let mut line = b"abcde".to_vec();
                let mut cursor = 2;
                let mut yank = Vec::new();
                replay_cmd(&mut line, &mut cursor, &mut yank, b'x', 2, None);
                assert_eq!(line, b"abe");
                assert_eq!(cursor, 2);

                let mut line = b"abcde".to_vec();
                cursor = 3;
                replay_cmd(&mut line, &mut cursor, &mut yank, b'X', 2, None);
                assert_eq!(line, b"ade");
                assert_eq!(cursor, 1);
            });
        }

        #[test]
        fn replay_cmd_r() {
            assert_no_syscalls(|| {
                let mut line = b"abcde".to_vec();
                let mut cursor = 1;
                let mut yank = Vec::new();
                replay_cmd(&mut line, &mut cursor, &mut yank, b'r', 3, Some(b'Z'));
                assert_eq!(line, b"aZZZe");
                assert_eq!(cursor, 3);
            });
        }

        #[test]
        fn replay_cmd_d_dd_and_motion() {
            assert_no_syscalls(|| {
                let mut line = b"hello world".to_vec();
                let mut cursor = 0;
                let mut yank = Vec::new();
                replay_cmd(&mut line, &mut cursor, &mut yank, b'd', 1, Some(b'w'));
                assert_eq!(line, b"world");
                assert_eq!(yank, b"hello ");

                let mut line = b"hello".to_vec();
                cursor = 0;
                replay_cmd(&mut line, &mut cursor, &mut yank, b'd', 1, Some(b'd'));
                assert!(line.is_empty());
                assert_eq!(cursor, 0);
            });
        }

        #[test]
        fn replay_cmd_c_cc_and_motion() {
            assert_no_syscalls(|| {
                let mut line = b"hello world".to_vec();
                let mut cursor = 0;
                let mut yank = Vec::new();
                replay_cmd(&mut line, &mut cursor, &mut yank, b'c', 1, Some(b'w'));
                assert_eq!(line, b"world");
                assert_eq!(cursor, 0);

                let mut line = b"hello".to_vec();
                cursor = 0;
                replay_cmd(&mut line, &mut cursor, &mut yank, b'c', 1, Some(b'c'));
                assert!(line.is_empty());
            });
        }

        #[test]
        fn replay_cmd_unknown_is_noop() {
            assert_no_syscalls(|| {
                let mut line = b"ab".to_vec();
                let mut cursor = 0;
                let mut yank = Vec::new();
                replay_cmd(&mut line, &mut cursor, &mut yank, b'z', 1, None);
                assert_eq!(line, b"ab");
            });
        }

        #[test]
        fn glob_expand_with_real_files() {
            let dir = std::env::temp_dir().join("meiksh_glob_test");
            let _ = std::fs::create_dir_all(&dir);
            std::fs::write(dir.join("aaa_1"), "").unwrap();
            std::fs::write(dir.join("aaa_2"), "").unwrap();
            let pat = format!("{}/aaa_*", dir.display());
            let result = glob_expand(pat.as_bytes());
            assert!(result.is_ok());
            let files = result.unwrap();
            assert_eq!(files.len(), 2);
            let _ = std::fs::remove_dir_all(&dir);
        }

        #[test]
        fn glob_expand_no_match_returns_err() {
            let result = glob_expand(b"/nonexistent_path_xyz/no_*_match");
            assert!(result.is_err());
        }

        #[test]
        fn vi_insert_mode_enter_returns_line() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                let actions = feed_bytes(&mut state, b"abc\n", &history);
                assert!(has_return(&actions));
                assert_eq!(get_return(&actions), Some(Some(b"abc\n".to_vec())));
            });
        }

        #[test]
        fn vi_insert_mode_eof_returns_none() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                let actions = state.process_byte(0x04, &history);
                assert!(has_return(&actions));
                assert_eq!(get_return(&actions), Some(None));
            });
        }

        #[test]
        fn vi_insert_mode_backspace_erases() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abc", &history);
                assert_eq!(state.line, b"abc");
                state.process_byte(0x7f, &history);
                assert_eq!(state.line, b"ab");
                assert_eq!(state.cursor, 2);
            });
        }

        #[test]
        fn vi_insert_mode_ctrl_c_returns_empty() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abc", &history);
                let actions = state.process_byte(0x03, &history);
                assert_eq!(get_return(&actions), Some(Some(Vec::new())));
            });
        }

        #[test]
        fn vi_insert_mode_ctrl_w_deletes_word() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"hello world", &history);
                state.process_byte(0x17, &history);
                assert_eq!(state.line, b"hello ");
            });
        }

        #[test]
        fn vi_insert_mode_ctrl_v_inserts_literal() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                state.process_byte(0x16, &history);
                state.process_byte(0x03, &history);
                assert_eq!(state.line, vec![0x03]);
            });
        }

        #[test]
        fn vi_esc_to_command_mode() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abc", &history);
                state.process_byte(0x1b, &history);
                assert!(!state.insert_mode);
                assert_eq!(state.cursor, 2);
            });
        }

        #[test]
        fn vi_command_h_l_motion() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abcde", &history);
                state.process_byte(0x1b, &history);
                assert_eq!(state.cursor, 4);
                state.process_byte(b'h', &history);
                assert_eq!(state.cursor, 3);
                state.process_byte(b'h', &history);
                assert_eq!(state.cursor, 2);
                state.process_byte(b'l', &history);
                assert_eq!(state.cursor, 3);
            });
        }

        #[test]
        fn vi_command_0_dollar() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abcde", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                assert_eq!(state.cursor, 0);
                state.process_byte(b'$', &history);
                assert_eq!(state.cursor, 4);
            });
        }

        #[test]
        fn vi_command_caret() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"  hello", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'^', &history);
                assert_eq!(state.cursor, 2);
            });
        }

        #[test]
        fn vi_command_w_b_motion() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"echo hello world", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                state.process_byte(b'w', &history);
                assert_eq!(state.cursor, 5);
                state.process_byte(b'w', &history);
                assert_eq!(state.cursor, 11);
                state.process_byte(b'b', &history);
                assert_eq!(state.cursor, 5);
            });
        }

        #[test]
        fn vi_command_W_B_motion() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"a.b c.d", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                state.process_byte(b'W', &history);
                assert_eq!(state.cursor, 4);
                state.process_byte(b'B', &history);
                assert_eq!(state.cursor, 0);
            });
        }

        #[test]
        fn vi_command_e_E_motion() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"ab cd", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                state.process_byte(b'e', &history);
                assert_eq!(state.cursor, 1);

                let mut state = ViState::new(0x7f, 0);
                feed_bytes(&mut state, b"a-b cd", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                state.process_byte(b'E', &history);
                assert_eq!(state.cursor, 2);
            });
        }

        #[test]
        fn vi_command_pipe() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abcde", &history);
                state.process_byte(0x1b, &history);
                feed_bytes(&mut state, b"3|", &history);
                assert_eq!(state.cursor, 2);
            });
        }

        #[test]
        fn vi_command_find_f_F_t_T() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abcba", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                feed_bytes(&mut state, b"fc", &history);
                assert_eq!(state.cursor, 2);
                feed_bytes(&mut state, b"Fb", &history);
                assert_eq!(state.cursor, 1);
                state.process_byte(b'0', &history);
                feed_bytes(&mut state, b"tc", &history);
                assert_eq!(state.cursor, 1);
                state.process_byte(b'$', &history);
                feed_bytes(&mut state, b"Tb", &history);
                assert_eq!(state.cursor, 4);
            });
        }

        #[test]
        fn vi_command_semicolon_comma_repeat_find() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"ababa", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                feed_bytes(&mut state, b"fa", &history);
                assert_eq!(state.cursor, 2);
                state.process_byte(b';', &history);
                assert_eq!(state.cursor, 4);
                state.process_byte(b',', &history);
                assert_eq!(state.cursor, 2);
            });
        }

        #[test]
        fn vi_command_x_delete() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abc", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                state.process_byte(b'x', &history);
                assert_eq!(state.line, b"bc");
            });
        }

        #[test]
        fn vi_command_X_delete_before() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abc", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'X', &history);
                assert_eq!(state.line, b"ac");
                assert_eq!(state.cursor, 1);
            });
        }

        #[test]
        fn vi_command_r_replace() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"hello", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                feed_bytes(&mut state, b"rH", &history);
                assert_eq!(state.line, b"Hello");
            });
        }

        #[test]
        fn vi_command_r_with_count() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abcd", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                feed_bytes(&mut state, b"3rZ", &history);
                assert_eq!(state.line, b"ZZZd");
            });
        }

        #[test]
        fn vi_command_R_replace_mode() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abcdef", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                feed_bytes(&mut state, b"RXY\x1b", &history);
                assert_eq!(state.line, b"XYcdef");
            });
        }

        #[test]
        fn vi_command_R_enter_returns() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"ab", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                state.process_byte(b'R', &history);
                let actions = state.process_byte(b'\n', &history);
                assert!(has_return(&actions));
            });
        }

        #[test]
        fn vi_command_tilde_toggle_case() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"aB", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                state.process_byte(b'~', &history);
                assert_eq!(state.line, b"AB");
                state.process_byte(b'~', &history);
                assert_eq!(state.line, b"Ab");
            });
        }

        #[test]
        fn vi_command_d_with_motion() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"hello world", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                feed_bytes(&mut state, b"dw", &history);
                assert_eq!(state.line, b"world");
                assert_eq!(state.yank_buf, b"hello ");
            });
        }

        #[test]
        fn vi_command_dd_clears_line() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"hello", &history);
                state.process_byte(0x1b, &history);
                feed_bytes(&mut state, b"dd", &history);
                assert!(state.line.is_empty());
            });
        }

        #[test]
        fn vi_command_D_delete_to_end() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"hello world", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                state.process_byte(b'w', &history);
                state.process_byte(b'D', &history);
                assert_eq!(state.line, b"hello ");
            });
        }

        #[test]
        fn vi_command_c_change() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"hello world", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                feed_bytes(&mut state, b"cw", &history);
                assert_eq!(state.line, b"world");
                assert!(state.insert_mode);
            });
        }

        #[test]
        fn vi_command_cc_clears_line() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"hello", &history);
                state.process_byte(0x1b, &history);
                feed_bytes(&mut state, b"cc", &history);
                assert!(state.line.is_empty());
                assert!(state.insert_mode);
            });
        }

        #[test]
        fn vi_command_C_change_to_end() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"hello world", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                state.process_byte(b'w', &history);
                state.process_byte(b'C', &history);
                assert_eq!(state.line, b"hello ");
                assert!(state.insert_mode);
            });
        }

        #[test]
        fn vi_command_S_substitute_line() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"hello", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'S', &history);
                assert!(state.line.is_empty());
                assert!(state.insert_mode);
            });
        }

        #[test]
        fn vi_command_y_yank() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"hello world", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                feed_bytes(&mut state, b"yw", &history);
                assert_eq!(state.yank_buf, b"hello ");
                assert_eq!(state.cursor, 0);
            });
        }

        #[test]
        fn vi_command_yy_yanks_line() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"hello", &history);
                state.process_byte(0x1b, &history);
                feed_bytes(&mut state, b"yy", &history);
                assert_eq!(state.yank_buf, b"hello");
            });
        }

        #[test]
        fn vi_command_Y_yank_to_end() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"hello world", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                state.process_byte(b'w', &history);
                state.process_byte(b'Y', &history);
                assert_eq!(state.yank_buf, b"world");
            });
        }

        #[test]
        fn vi_command_p_P_put() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abc", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'x', &history);
                assert_eq!(state.yank_buf, vec![b'c']);
                state.process_byte(b'p', &history);
                assert_eq!(state.line, b"abc");

                let mut state = ViState::new(0x7f, 0);
                feed_bytes(&mut state, b"abc", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'x', &history);
                state.process_byte(b'P', &history);
                assert_eq!(state.line, b"acb");
            });
        }

        #[test]
        fn vi_command_u_undo() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"hello", &history);
                state.process_byte(0x1b, &history);
                state.edit_line = state.line.clone();
                state.process_byte(b'x', &history);
                assert_eq!(state.line, b"hell");
                state.process_byte(b'u', &history);
                assert_eq!(state.line, b"hello");
            });
        }

        #[test]
        fn vi_command_dot_repeat() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abcde", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                state.process_byte(b'x', &history);
                assert_eq!(state.line, b"bcde");
                state.process_byte(b'.', &history);
                assert_eq!(state.line, b"cde");
            });
        }

        #[test]
        fn vi_command_a_A_i_I_enter_insert() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"ab", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                state.process_byte(b'a', &history);
                assert!(state.insert_mode);
                assert_eq!(state.cursor, 1);

                state.process_byte(0x1b, &history);
                state.process_byte(b'A', &history);
                assert!(state.insert_mode);
                assert_eq!(state.cursor, 2);

                state.process_byte(0x1b, &history);
                state.process_byte(b'I', &history);
                assert!(state.insert_mode);
                assert_eq!(state.cursor, 0);

                state.process_byte(0x1b, &history);
                state.process_byte(b'i', &history);
                assert!(state.insert_mode);
            });
        }

        #[test]
        fn vi_command_history_k_j() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![b"cmd1"[..].into(), b"cmd2"[..].into()];
                let mut state = ViState::new(0x7f, history.len());
                feed_bytes(&mut state, b"current", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'k', &history);
                assert_eq!(state.line, b"cmd2");
                assert_eq!(state.hist_index, Some(1));
                state.process_byte(b'k', &history);
                assert_eq!(state.line, b"cmd1");
                assert_eq!(state.hist_index, Some(0));
                let actions = state.process_byte(b'k', &history);
                assert!(has_bell(&actions));
                state.process_byte(b'j', &history);
                assert_eq!(state.line, b"cmd2");
                state.process_byte(b'j', &history);
                assert_eq!(state.line, b"current");
                assert_eq!(state.hist_index, None);
            });
        }

        #[test]
        fn vi_command_G_goes_to_oldest() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![b"oldest"[..].into(), b"newest"[..].into()];
                let mut state = ViState::new(0x7f, history.len());
                feed_bytes(&mut state, b"cur", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'G', &history);
                assert_eq!(state.line, b"oldest");
            });
        }

        #[test]
        fn vi_command_hash_comments_out() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"echo hello", &history);
                state.process_byte(0x1b, &history);
                let actions = state.process_byte(b'#', &history);
                assert!(has_return(&actions));
                let ret = get_return(&actions).unwrap().unwrap();
                assert!(ret.starts_with(b"#echo hello"));
            });
        }

        #[test]
        fn vi_command_sigint_in_command_mode() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"partial", &history);
                state.process_byte(0x1b, &history);
                let actions = state.process_byte(0x03, &history);
                assert_eq!(get_return(&actions), Some(Some(Vec::new())));
            });
        }

        #[test]
        fn vi_command_enter_in_command_mode() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"cmd", &history);
                state.process_byte(0x1b, &history);
                let actions = state.process_byte(b'\n', &history);
                assert!(has_return(&actions));
                assert_eq!(get_return(&actions), Some(Some(b"cmd\n".to_vec())));
            });
        }

        #[test]
        fn vi_command_U_undoes_all() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![b"baseline"[..].into()];
                let mut state = ViState::new(0x7f, history.len());
                feed_bytes(&mut state, b"cur", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'k', &history);
                assert_eq!(state.line, b"baseline");
                state.process_byte(b'$', &history);
                state.process_byte(b'x', &history);
                assert_eq!(state.line, b"baselin");
                state.process_byte(b'U', &history);
                assert_eq!(state.line, b"baseline");
            });
        }

        #[test]
        fn vi_command_minus_navigates_history() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![b"hist1"[..].into()];
                let mut state = ViState::new(0x7f, history.len());
                state.process_byte(0x1b, &history);
                state.process_byte(b'-', &history);
                assert_eq!(state.line, b"hist1");
            });
        }

        #[test]
        fn vi_command_plus_navigates_forward() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![b"h1"[..].into(), b"h2"[..].into()];
                let mut state = ViState::new(0x7f, history.len());
                state.process_byte(0x1b, &history);
                state.process_byte(b'k', &history);
                state.process_byte(b'+', &history);
                assert_eq!(state.hist_index, None);
            });
        }

        #[test]
        fn vi_command_search_backward() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![b"alpha"[..].into(), b"beta"[..].into()];
                let mut state = ViState::new(0x7f, history.len());
                state.process_byte(0x1b, &history);
                feed_bytes(&mut state, b"/alp\n", &history);
                assert_eq!(state.line, b"alpha");
            });
        }

        #[test]
        fn vi_command_search_forward() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![b"alpha"[..].into(), b"beta"[..].into()];
                let mut state = ViState::new(0x7f, history.len());
                state.process_byte(0x1b, &history);
                state.process_byte(b'k', &history);
                state.process_byte(b'k', &history);
                feed_bytes(&mut state, b"?beta\n", &history);
                assert_eq!(state.line, b"beta");
            });
        }

        #[test]
        fn vi_command_n_N_repeat_search() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![
                    b"alpha1"[..].into(),
                    b"beta"[..].into(),
                    b"alpha2"[..].into(),
                ];
                let mut state = ViState::new(0x7f, history.len());
                state.process_byte(0x1b, &history);
                feed_bytes(&mut state, b"/alpha\n", &history);
                assert_eq!(state.line, b"alpha2");
                state.process_byte(b'n', &history);
                assert_eq!(state.line, b"alpha1");
                state.process_byte(b'N', &history);
                assert_eq!(state.line, b"alpha2");
            });
        }

        #[test]
        fn vi_command_search_not_found_bells() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![b"alpha"[..].into()];
                let mut state = ViState::new(0x7f, history.len());
                state.process_byte(0x1b, &history);
                let actions = feed_bytes(&mut state, b"/zzz\n", &history);
                assert!(has_bell(&actions));
            });
        }

        #[test]
        fn vi_command_search_backspace() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![b"alpha"[..].into()];
                let mut state = ViState::new(0x7f, history.len());
                state.process_byte(0x1b, &history);
                feed_bytes(&mut state, b"/alphx\x7fa\n", &history);
                assert_eq!(state.line, b"alpha");
            });
        }

        #[test]
        fn vi_command_d_with_invalid_motion_bells() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"hello", &history);
                state.process_byte(0x1b, &history);
                let actions = feed_bytes(&mut state, b"dz", &history);
                assert!(has_bell(&actions));
            });
        }

        #[test]
        fn vi_command_unknown_bells() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"x", &history);
                state.process_byte(0x1b, &history);
                let actions = state.process_byte(b'Z', &history);
                assert!(has_bell(&actions));
            });
        }

        #[test]
        fn vi_command_count_prefix() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abcde", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                feed_bytes(&mut state, b"3x", &history);
                assert_eq!(state.line, b"de");
            });
        }

        #[test]
        fn vi_command_numbered_G() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> =
                    vec![b"h0"[..].into(), b"h1"[..].into(), b"h2"[..].into()];
                let mut state = ViState::new(0x7f, history.len());
                state.process_byte(0x1b, &history);
                feed_bytes(&mut state, b"2G", &history);
                assert_eq!(state.line, b"h1");
            });
        }

        #[test]
        fn vi_command_numbered_G_empty_history() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![];
                let mut state = ViState::new(0x7f, history.len());
                state.process_byte(0x1b, &history);
                feed_bytes(&mut state, b"2G", &history);
                assert_eq!(state.line, b"");
            });
        }

        #[test]
        fn vi_command_v_returns_run_editor_action() {
            run_trace(
                trace_entries![
                    getpid() -> 42,
                    open(_, _, _) -> 10,
                    write(fd(10), _) -> auto,
                    write(fd(10), _) -> auto,
                    close(fd(10)) -> 0,
                ],
                || {
                    let mut state = ViState::new(0x7f, 0);
                    let history: Vec<Box<[u8]>> = vec![];
                    state.line = b"hello".to_vec();
                    state.cursor = 4;
                    state.insert_mode = false;
                    let actions = state.process_byte(b'v', &history);
                    assert!(
                        actions
                            .iter()
                            .any(|a| matches!(a, ViAction::RunEditor { .. }))
                    );
                },
            );
        }

        #[test]
        fn vi_h_at_beginning_bells() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"a", &history);
                state.process_byte(0x1b, &history);
                let actions = state.process_byte(b'h', &history);
                assert!(has_bell(&actions));
            });
        }

        #[test]
        fn vi_l_at_end_bells() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"a", &history);
                state.process_byte(0x1b, &history);
                let actions = state.process_byte(b'l', &history);
                assert!(has_bell(&actions));
            });
        }

        #[test]
        fn vi_w_at_end_no_move() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abc", &history);
                state.process_byte(0x1b, &history);
                let before = state.cursor;
                let _actions = state.process_byte(b'w', &history);
                assert_eq!(state.cursor, before);
            });
        }

        #[test]
        fn vi_b_at_start_bells() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abc", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                let actions = state.process_byte(b'b', &history);
                assert!(has_bell(&actions));
            });
        }

        #[test]
        fn vi_W_at_end_no_move() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abc", &history);
                state.process_byte(0x1b, &history);
                let before = state.cursor;
                let _actions = state.process_byte(b'W', &history);
                assert_eq!(state.cursor, before);
            });
        }

        #[test]
        fn vi_B_at_start_bells() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abc", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                let actions = state.process_byte(b'B', &history);
                assert!(has_bell(&actions));
            });
        }

        #[test]
        fn vi_find_not_found_bells() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abc", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                let actions = feed_bytes(&mut state, b"fz", &history);
                assert!(has_bell(&actions));
            });
        }

        #[test]
        fn vi_X_at_start_bells() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"a", &history);
                state.process_byte(0x1b, &history);
                let actions = state.process_byte(b'X', &history);
                assert!(has_bell(&actions));
            });
        }

        #[test]
        fn vi_j_with_no_history_bells() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![];
                let mut state = ViState::new(0x7f, 0);
                state.process_byte(0x1b, &history);
                let actions = state.process_byte(b'j', &history);
                assert!(has_bell(&actions));
            });
        }

        #[test]
        fn vi_k_with_no_history_bells() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![];
                let mut state = ViState::new(0x7f, 0);
                state.process_byte(0x1b, &history);
                let actions = state.process_byte(b'k', &history);
                assert!(has_bell(&actions));
            });
        }

        #[test]
        fn vi_insert_not_at_end_redraws() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"ac", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                state.process_byte(b'l', &history);
                state.process_byte(b'i', &history);
                let actions = state.process_byte(b'b', &history);
                assert_eq!(state.line, b"abc");
                assert!(actions.iter().any(|a| matches!(a, ViAction::Redraw)));
            });
        }

        #[test]
        fn vi_tilde_count_overflow() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"aB", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                feed_bytes(&mut state, b"9~", &history);
                assert_eq!(state.line, b"Ab");
            });
        }

        #[test]
        fn word_backward_skips_punctuation_class() {
            assert_no_syscalls(|| {
                assert_eq!(word_backward(b"abc...", 5), 3);
                assert_eq!(word_backward(b"   ", 2), 0);
            });
        }

        #[test]
        fn word_end_punctuation_class() {
            assert_no_syscalls(|| {
                assert_eq!(word_end(b"abc...xyz", 0), 2);
                assert_eq!(word_end(b"abc...xyz", 3), 5);
                assert_eq!(word_end(b"a  ", 0), 2);
            });
        }

        #[test]
        fn bigword_end_at_end() {
            assert_no_syscalls(|| {
                assert_eq!(bigword_end(b"abc", 2), 2);
                assert_eq!(bigword_end(b"a  ", 0), 2);
            });
        }

        #[test]
        fn vi_count_digits_continuation() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> =
                    vec![b"one"[..].into(), b"two"[..].into(), b"three"[..].into()];
                let mut state = ViState::new(0x7f, history.len());
                feed_bytes(&mut state, b"text", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'1', &history);
                state.process_byte(b'2', &history);
                state.process_byte(b'G', &history);
                assert!(!state.line.is_empty());
            });
        }

        #[test]
        fn vi_replace_with_count() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abcdef", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                state.process_byte(b'3', &history);
                state.process_byte(b'r', &history);
                state.process_byte(b'z', &history);
                assert_eq!(&state.line[..3], b"zzz");
            });
        }

        #[test]
        fn vi_replace_mode_esc_adjusts_cursor() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abc", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'R', &history);
                state.process_byte(b'z', &history);
                state.process_byte(b'z', &history);
                state.process_byte(b'z', &history);
                state.process_byte(0x1b, &history);
                assert_eq!(state.line, b"abzzz");
                assert_eq!(state.cursor, 4);
            });
        }

        #[test]
        fn vi_replace_mode_past_end() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"ab", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'R', &history);
                state.process_byte(b'x', &history);
                state.process_byte(b'y', &history);
                state.process_byte(b'z', &history);
                assert_eq!(state.line, b"axyz");
            });
        }

        #[test]
        fn vi_count_zero_normalization() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abc", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'i', &history);
                assert!(state.insert_mode);
            });
        }

        #[test]
        fn vi_semicolon_bell_on_not_found() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abcdef", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                state.process_byte(b'f', &history);
                state.process_byte(b'c', &history);
                assert_eq!(state.cursor, 2);
                let actions = state.process_byte(b';', &history);
                assert!(has_bell(&actions));
            });
        }

        #[test]
        fn vi_comma_reverses_find_direction() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abcba", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                state.process_byte(b'f', &history);
                state.process_byte(b'b', &history);
                assert_eq!(state.cursor, 1);
                state.process_byte(b';', &history);
                assert_eq!(state.cursor, 3);
                state.process_byte(b',', &history);
                assert_eq!(state.cursor, 1);
            });
        }

        #[test]
        fn vi_comma_bell_when_not_found() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abcdef", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'$', &history);
                state.process_byte(b'F', &history);
                state.process_byte(b'c', &history);
                assert_eq!(state.cursor, 2);
                let actions = state.process_byte(b',', &history);
                assert!(has_bell(&actions));
            });
        }

        #[test]
        fn vi_D_on_empty_line() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                state.process_byte(0x1b, &history);
                let _actions = state.process_byte(b'D', &history);
                assert!(state.line.is_empty());
            });
        }

        #[test]
        fn vi_p_empty_yank_buf() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abc", &history);
                state.process_byte(0x1b, &history);
                let actions = state.process_byte(b'p', &history);
                assert_eq!(state.line, b"abc");
                assert!(!actions.iter().any(|a| matches!(a, ViAction::Redraw)));
            });
        }

        #[test]
        fn vi_P_empty_yank_buf() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abc", &history);
                state.process_byte(0x1b, &history);
                let actions = state.process_byte(b'P', &history);
                assert_eq!(state.line, b"abc");
                assert!(!actions.iter().any(|a| matches!(a, ViAction::Redraw)));
            });
        }

        #[test]
        fn vi_U_without_history_clears_line() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"some text", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'U', &history);
                assert!(state.line.is_empty());
                assert_eq!(state.cursor, 0);
            });
        }

        #[test]
        fn vi_dot_with_explicit_count() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abcdef", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'x', &history);
                state.process_byte(b'2', &history);
                state.process_byte(b'.', &history);
                assert_eq!(state.line.len(), 3);
            });
        }

        #[test]
        fn vi_dot_no_last_cmd() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abc", &history);
                state.process_byte(0x1b, &history);
                let _actions = state.process_byte(b'.', &history);
                assert_eq!(state.line, b"abc");
            });
        }

        #[test]
        fn vi_k_with_empty_history_line() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![b""[..].into()];
                let mut state = ViState::new(0x7f, history.len());
                feed_bytes(&mut state, b"cur", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'k', &history);
                assert_eq!(state.cursor, 0);
                assert!(state.line.is_empty());
            });
        }

        #[test]
        fn vi_G_with_explicit_count() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![
                    b"first"[..].into(),
                    b"second"[..].into(),
                    b"third"[..].into(),
                ];
                let mut state = ViState::new(0x7f, history.len());
                feed_bytes(&mut state, b"cur", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'2', &history);
                state.process_byte(b'G', &history);
                assert_eq!(state.line, b"second");
            });
        }

        #[test]
        fn vi_G_default_goes_to_oldest() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![
                    b"first"[..].into(),
                    b"second"[..].into(),
                    b"third"[..].into(),
                ];
                let mut state = ViState::new(0x7f, history.len());
                feed_bytes(&mut state, b"cur", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'G', &history);
                assert_eq!(state.line, b"first");
                assert_eq!(state.cursor, 4);
            });
        }

        #[test]
        fn vi_G_with_empty_history_line() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![b""[..].into()];
                let mut state = ViState::new(0x7f, history.len());
                feed_bytes(&mut state, b"x", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'G', &history);
                assert!(state.line.is_empty());
                assert_eq!(state.cursor, 0);
            });
        }

        #[test]
        fn vi_search_forward_break_and_edit_line() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![b"alpha"[..].into(), b"beta"[..].into()];
                let mut state = ViState::new(0x7f, history.len());
                feed_bytes(&mut state, b"cur", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'/', &history);
                for &b in b"alpha" {
                    state.process_byte(b, &history);
                }
                state.process_byte(b'\r', &history);
                assert_eq!(state.line, b"alpha");
            });
        }

        #[test]
        fn vi_search_backward_not_found_bells() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![b"alpha"[..].into()];
                let mut state = ViState::new(0x7f, history.len());
                feed_bytes(&mut state, b"cur", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'k', &history);
                assert_eq!(state.line, b"alpha");
                state.process_byte(b'?', &history);
                for &b in b"nothere" {
                    state.process_byte(b, &history);
                }
                let actions = state.process_byte(b'\r', &history);
                assert!(has_bell(&actions));
            });
        }

        #[test]
        fn vi_w_truly_stuck_no_movement() {
            assert_no_syscalls(|| {
                let next = word_forward(b"a", 0);
                assert_eq!(next, 1);
                let clamped = next.min(1usize.saturating_sub(1));
                assert_eq!(clamped, 0);
            });
        }

        #[test]
        fn replay_cmd_x_cursor_adjusts() {
            assert_no_syscalls(|| {
                let mut line = b"ab".to_vec();
                let mut cursor = 1usize;
                let mut yank = vec![];
                replay_cmd(&mut line, &mut cursor, &mut yank, b'x', 2, None);
                assert_eq!(line, b"");
                assert_eq!(cursor, 0);
            });
        }

        #[test]
        fn replay_cmd_r_with_count() {
            assert_no_syscalls(|| {
                let mut line = b"abcdef".to_vec();
                let mut cursor = 0usize;
                let mut yank = vec![];
                replay_cmd(&mut line, &mut cursor, &mut yank, b'r', 3, Some(b'z'));
                assert_eq!(&line[..3], b"zzz");
            });
        }

        #[test]
        fn replay_cmd_d_and_c_with_motion() {
            assert_no_syscalls(|| {
                let mut line = b"hello world".to_vec();
                let mut cursor = 0usize;
                let mut yank = vec![];
                replay_cmd(&mut line, &mut cursor, &mut yank, b'd', 1, Some(b'w'));
                assert_eq!(line, b"world");

                let mut line2 = b"hello world".to_vec();
                let mut cursor2 = 0usize;
                let mut yank2 = vec![];
                replay_cmd(&mut line2, &mut cursor2, &mut yank2, b'c', 1, Some(b'w'));
                assert_eq!(line2, b"world");
            });
        }

        #[test]
        fn vi_star_glob_expand() {
            assert_no_syscalls(|| {
                let dir = std::env::temp_dir().join("meiksh_vi_star_test");
                let _ = std::fs::remove_dir_all(&dir);
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(dir.join("aaa.txt"), b"").unwrap();
                std::fs::write(dir.join("bbb.txt"), b"").unwrap();

                let pattern = format!("{}/", dir.display());
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                state.line = pattern.as_bytes().to_vec();
                state.cursor = state.line.len().saturating_sub(1);
                state.insert_mode = false;
                state.process_byte(b'*', &history);
                assert!(
                    state
                        .line
                        .windows(b"aaa.txt".len())
                        .any(|w| w == b"aaa.txt")
                );
                assert!(
                    state
                        .line
                        .windows(b"bbb.txt".len())
                        .any(|w| w == b"bbb.txt")
                );
                let _ = std::fs::remove_dir_all(&dir);
            });
        }

        #[test]
        fn vi_backslash_unique_completion() {
            let dir = std::env::temp_dir().join("meiksh_vi_bslash_test");
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("unique_file.txt"), b"").unwrap();

            let expected = format!("{}/unique_file.txt", dir.display());
            run_trace(
                trace_entries![
                    stat(str(expected), _) -> stat_file(0o644),
                ],
                || {
                    let prefix = format!("{}/unique_fi", dir.display());
                    let mut state = ViState::new(0x7f, 0);
                    let history: Vec<Box<[u8]>> = vec![];
                    state.line = prefix.as_bytes().to_vec();
                    state.cursor = state.line.len().saturating_sub(1);
                    state.insert_mode = false;
                    state.process_byte(b'\\', &history);
                    assert!(
                        state
                            .line
                            .windows(b"unique_file.txt".len())
                            .any(|w| w == b"unique_file.txt")
                    );
                },
            );
            let _ = std::fs::remove_dir_all(&dir);
        }

        #[test]
        fn vi_backslash_dir_appends_slash() {
            let dir = std::env::temp_dir().join("meiksh_vi_bslash_dir_test");
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("subdir_only")).unwrap();

            let expected = format!("{}/subdir_only/", dir.display());
            run_trace(
                trace_entries![
                    stat(str(expected), _) -> stat_dir,
                ],
                || {
                    let prefix = format!("{}/subdir_on", dir.display());
                    let mut state = ViState::new(0x7f, 0);
                    let history: Vec<Box<[u8]>> = vec![];
                    state.line = prefix.as_bytes().to_vec();
                    state.cursor = state.line.len().saturating_sub(1);
                    state.insert_mode = false;
                    state.process_byte(b'\\', &history);
                    assert_eq!(state.line.last(), Some(&b'/'));
                },
            );
            let _ = std::fs::remove_dir_all(&dir);
        }

        #[test]
        fn vi_backslash_ambiguous_bells() {
            assert_no_syscalls(|| {
                let dir = std::env::temp_dir().join("meiksh_vi_bslash_amb_test");
                let _ = std::fs::remove_dir_all(&dir);
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(dir.join("ab1.txt"), b"").unwrap();
                std::fs::write(dir.join("ab2.txt"), b"").unwrap();

                let prefix = format!("{}/ab", dir.display());
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                state.line = prefix.as_bytes().to_vec();
                state.cursor = state.line.len().saturating_sub(1);
                state.insert_mode = false;
                let actions = state.process_byte(b'\\', &history);
                assert!(has_bell(&actions));
                let _ = std::fs::remove_dir_all(&dir);
            });
        }

        #[test]
        fn glob_expand_error_returns_err() {
            assert_no_syscalls(|| {
                assert!(glob_expand(b"\0invalid").is_err());
            });
        }

        #[test]
        fn vi_r_replace_at_end_of_line() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"a", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'3', &history);
                state.process_byte(b'r', &history);
                state.process_byte(b'z', &history);
                assert_eq!(state.line, b"z");
            });
        }

        #[test]
        fn vi_w_empty_line_truly_stuck() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                state.insert_mode = false;
                let actions = state.process_byte(b'w', &history);
                assert!(has_bell(&actions));
            });
        }

        #[test]
        fn vi_W_empty_line_truly_stuck() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                state.insert_mode = false;
                let actions = state.process_byte(b'W', &history);
                assert!(has_bell(&actions));
            });
        }

        #[test]
        fn vi_comma_with_t_and_T_directions() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abcba", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                state.process_byte(b't', &history);
                state.process_byte(b'b', &history);
                let saved = state.cursor;
                state.process_byte(b',', &history);
                let _ = saved;

                let mut state2 = ViState::new(0x7f, 0);
                feed_bytes(&mut state2, b"abcba", &history);
                state2.process_byte(0x1b, &history);
                state2.process_byte(b'$', &history);
                state2.process_byte(b'T', &history);
                state2.process_byte(b'b', &history);
                state2.process_byte(b',', &history);
            });
        }

        #[test]
        fn vi_tilde_at_end_break() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"a", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'~', &history);
                assert_eq!(state.line, b"A");
            });
        }

        #[test]
        fn vi_D_on_empty() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                state.insert_mode = false;
                state.process_byte(b'D', &history);
                assert!(state.line.is_empty());
            });
        }

        #[test]
        fn vi_p_P_empty_yank_no_redraw() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"x", &history);
                state.process_byte(0x1b, &history);
                let a1 = state.process_byte(b'p', &history);
                assert!(!a1.iter().any(|a| matches!(a, ViAction::Redraw)));
                let a2 = state.process_byte(b'P', &history);
                assert!(!a2.iter().any(|a| matches!(a, ViAction::Redraw)));
            });
        }

        #[test]
        fn vi_star_with_explicit_glob_chars() {
            assert_no_syscalls(|| {
                let dir = std::env::temp_dir().join("meiksh_vi_star_glob_test");
                let _ = std::fs::remove_dir_all(&dir);
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(dir.join("file1.txt"), b"").unwrap();

                let pattern = format!("{}/*.txt", dir.display());
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                state.line = pattern.as_bytes().to_vec();
                state.cursor = state.line.len().saturating_sub(1);
                state.insert_mode = false;
                state.process_byte(b'*', &history);
                assert!(
                    state
                        .line
                        .windows(b"file1.txt".len())
                        .any(|w| w == b"file1.txt")
                );
                let _ = std::fs::remove_dir_all(&dir);
            });
        }

        #[test]
        fn vi_search_forward_idx_break() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![b"aaa"[..].into()];
                let mut state = ViState::new(0x7f, history.len());
                feed_bytes(&mut state, b"x", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'/', &history);
                for &b in b"aaa" {
                    state.process_byte(b, &history);
                }
                state.process_byte(b'\r', &history);
                assert_eq!(state.line, b"aaa");
            });
        }

        #[test]
        fn vi_search_backward_not_found() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![b"aaa"[..].into()];
                let mut state = ViState::new(0x7f, history.len());
                feed_bytes(&mut state, b"x", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'k', &history);
                state.process_byte(b'?', &history);
                for &b in b"zzz" {
                    state.process_byte(b, &history);
                }
                let actions = state.process_byte(b'\r', &history);
                assert!(has_bell(&actions));
            });
        }

        #[test]
        fn replay_cmd_r_past_end() {
            assert_no_syscalls(|| {
                let mut line = b"a".to_vec();
                let mut cursor = 0usize;
                let mut yank = vec![];
                replay_cmd(&mut line, &mut cursor, &mut yank, b'r', 5, Some(b'z'));
                assert_eq!(line, b"z");
            });
        }

        #[test]
        fn replay_cmd_d_with_dd() {
            assert_no_syscalls(|| {
                let mut line = b"hello".to_vec();
                let mut cursor = 0usize;
                let mut yank = vec![];
                replay_cmd(&mut line, &mut cursor, &mut yank, b'd', 1, Some(b'd'));
                assert!(line.is_empty());
                assert_eq!(cursor, 0);
            });
        }

        #[test]
        fn replay_cmd_c_with_cc() {
            assert_no_syscalls(|| {
                let mut line = b"hello".to_vec();
                let mut cursor = 0usize;
                let mut yank = vec![];
                replay_cmd(&mut line, &mut cursor, &mut yank, b'c', 1, Some(b'c'));
                assert!(line.is_empty());
                assert_eq!(cursor, 0);
            });
        }

        #[test]
        fn vi_semicolon_with_last_find_on_end() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"a", &history);
                state.process_byte(0x1b, &history);
                state.last_find = Some((b'f', b'z' as u32));
                let actions = state.process_byte(b';', &history);
                assert!(has_bell(&actions));
                state.last_find = Some((b'f', b'z' as u32));
                let _actions = state.process_byte(b';', &history);
            });
        }

        #[test]
        fn vi_semicolon_no_last_find() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abc", &history);
                state.process_byte(0x1b, &history);
                let actions = state.process_byte(b';', &history);
                assert!(!has_bell(&actions));
            });
        }

        #[test]
        fn vi_comma_reverse_find() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abcabc", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                feed_bytes(&mut state, b"fb", &history);
                assert_eq!(state.cursor, 1);
                state.process_byte(b';', &history);
                assert_eq!(state.cursor, 4);
                state.process_byte(b',', &history);
                assert_eq!(state.cursor, 1);
            });
        }

        #[test]
        fn vi_comma_no_last_find() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abc", &history);
                state.process_byte(0x1b, &history);
                let actions = state.process_byte(b',', &history);
                assert!(!has_bell(&actions));
            });
        }

        #[test]
        fn vi_comma_with_invalid_last_find_cmd() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"abc", &history);
                state.process_byte(0x1b, &history);
                state.last_find = Some((b'z', b'a' as u32));
                let actions = state.process_byte(b',', &history);
                assert!(has_bell(&actions));
            });
        }

        #[test]
        fn vi_replace_char_past_end() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                state.process_byte(0x1b, &history);
                feed_bytes(&mut state, b"rZ", &history);
                assert_eq!(state.line, b"");
            });
        }

        #[test]
        fn vi_tilde_on_empty_line() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                state.process_byte(0x1b, &history);
                state.process_byte(b'~', &history);
                assert!(state.line.is_empty());
            });
        }

        #[test]
        fn vi_x_past_end() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                state.process_byte(0x1b, &history);
                state.process_byte(b'x', &history);
                assert!(state.line.is_empty());
            });
        }

        #[test]
        fn vi_G_with_count() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![
                    b"first"[..].into(),
                    b"second"[..].into(),
                    b"third"[..].into(),
                ];
                let mut state = ViState::new(0x7f, history.len());
                feed_bytes(&mut state, b"current", &history);
                state.process_byte(0x1b, &history);
                feed_bytes(&mut state, b"2G", &history);
                assert_eq!(state.line, b"second");
                assert!(state.hist_index.is_some());
            });
        }

        #[test]
        fn vi_G_without_count_no_history() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![];
                let mut state = ViState::new(0x7f, history.len());
                feed_bytes(&mut state, b"text", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'G', &history);
                assert_eq!(state.line, b"text");
            });
        }

        #[test]
        fn vi_G_without_count_with_history() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![b"oldest"[..].into(), b"newest"[..].into()];
                let mut state = ViState::new(0x7f, history.len());
                feed_bytes(&mut state, b"text", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'G', &history);
                assert_eq!(state.line, b"oldest");
            });
        }

        #[test]
        fn vi_search_forward_finds_match() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![
                    b"echo hello"[..].into(),
                    b"ls -la"[..].into(),
                    b"echo world"[..].into(),
                ];
                let mut state = ViState::new(0x7f, history.len());
                feed_bytes(&mut state, b"x", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'/', &history);
                for &b in b"echo" {
                    state.process_byte(b, &history);
                }
                state.process_byte(b'\r', &history);
                assert!(state.hist_index.is_some());
                let idx = state.hist_index.unwrap();
                assert!(history[idx].windows(4).any(|w| w == b"echo"));
            });
        }

        #[test]
        fn vi_search_forward_not_found() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![b"aaa"[..].into(), b"bbb"[..].into()];
                let mut state = ViState::new(0x7f, history.len());
                feed_bytes(&mut state, b"x", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'/', &history);
                for &b in b"zzz" {
                    state.process_byte(b, &history);
                }
                let actions = state.process_byte(b'\r', &history);
                assert!(has_bell(&actions));
            });
        }

        #[test]
        fn vi_search_forward_idx_wraps() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![b"alpha"[..].into(), b"beta"[..].into()];
                let mut state = ViState::new(0x7f, history.len());
                feed_bytes(&mut state, b"x", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'k', &history);
                state.process_byte(b'/', &history);
                for &b in b"alpha" {
                    state.process_byte(b, &history);
                }
                state.process_byte(b'\r', &history);
                assert_eq!(state.hist_index, Some(0));
            });
        }

        #[test]
        fn vi_search_backward_finds_match() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![
                    b"echo hello"[..].into(),
                    b"ls -la"[..].into(),
                    b"echo world"[..].into(),
                ];
                let mut state = ViState::new(0x7f, history.len());
                feed_bytes(&mut state, b"x", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'?', &history);
                for &b in b"echo" {
                    state.process_byte(b, &history);
                }
                state.process_byte(b'\r', &history);
                assert!(state.hist_index.is_some());
                let idx = state.hist_index.unwrap();
                assert!(history[idx].windows(4).any(|w| w == b"echo"));
            });
        }

        #[test]
        fn vi_search_default_direction_noop() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![b"aaa"[..].into()];
                let mut state = ViState::new(0x7f, history.len());
                feed_bytes(&mut state, b"x", &history);
                state.process_byte(0x1b, &history);
                let mut actions = Vec::new();
                state.do_search(b'x', &history, &mut actions);
                assert!(actions.is_empty());
            });
        }

        #[test]
        fn replay_cmd_r_no_arg() {
            assert_no_syscalls(|| {
                let mut line = b"abc".to_vec();
                let mut cursor = 0usize;
                let mut yank = vec![];
                replay_cmd(&mut line, &mut cursor, &mut yank, b'r', 1, None);
                assert_eq!(line, b"abc");
            });
        }

        #[test]
        fn replay_cmd_r_cursor_past_end() {
            assert_no_syscalls(|| {
                let mut line = vec![];
                let mut cursor = 0usize;
                let mut yank = vec![];
                replay_cmd(&mut line, &mut cursor, &mut yank, b'r', 1, Some(b'z'));
                assert!(line.is_empty());
            });
        }

        #[test]
        fn replay_cmd_d_no_arg() {
            assert_no_syscalls(|| {
                let mut line = b"hello".to_vec();
                let mut cursor = 0usize;
                let mut yank = vec![];
                replay_cmd(&mut line, &mut cursor, &mut yank, b'd', 1, None);
                assert_eq!(line, b"hello");
            });
        }

        #[test]
        fn replay_cmd_c_no_arg() {
            assert_no_syscalls(|| {
                let mut line = b"hello".to_vec();
                let mut cursor = 0usize;
                let mut yank = vec![];
                replay_cmd(&mut line, &mut cursor, &mut yank, b'c', 1, None);
                assert_eq!(line, b"hello");
            });
        }

        #[test]
        fn replay_cmd_d_with_motion() {
            assert_no_syscalls(|| {
                let mut line = b"hello world".to_vec();
                let mut cursor = 0usize;
                let mut yank = vec![];
                replay_cmd(&mut line, &mut cursor, &mut yank, b'd', 1, Some(b'w'));
                assert_eq!(line, b"world");
                assert_eq!(yank, b"hello ");
            });
        }

        #[test]
        fn replay_cmd_c_with_motion() {
            assert_no_syscalls(|| {
                let mut line = b"hello world".to_vec();
                let mut cursor = 0usize;
                let mut yank = vec![];
                replay_cmd(&mut line, &mut cursor, &mut yank, b'c', 1, Some(b'w'));
                assert_eq!(line, b"world");
                assert_eq!(yank, b"hello ");
            });
        }

        #[test]
        fn glob_expand_null_byte_returns_err() {
            assert_no_syscalls(|| {
                let result = glob_expand(b"foo\0bar");
                assert!(result.is_err());
            });
        }

        #[test]
        fn vi_process_motion_unknown_op_noop() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![];
                let mut state = ViState::new(0x7f, history.len());
                feed_bytes(&mut state, b"abc", &history);
                state.process_byte(0x1b, &history);
                state.pending = PendingInput::Motion { op: b'z', count: 1 };
                let actions = state.process_byte(b'w', &history);
                assert_eq!(state.line, b"abc");
                assert!(actions.is_empty() || !has_bell(&actions));
            });
        }

        #[test]
        fn glob_expand_nomatch_returns_err() {
            assert_no_syscalls(|| {
                let result = glob_expand(b"/nonexistent_dir_xyz_42/*.qqq");
                assert!(result.is_err());
            });
        }

        #[test]
        fn vi_star_glob_nomatch_leaves_line() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                let word = b"/no_such_dir_xyzzy/";
                state.line = word.to_vec();
                state.cursor = state.line.len().saturating_sub(1);
                state.insert_mode = false;
                state.process_byte(b'*', &history);
                assert_eq!(state.line, word);
            });
        }

        #[test]
        fn vi_search_forward_from_oldest_wraps() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![b"alpha"[..].into(), b"beta"[..].into()];
                let mut state = ViState::new(0x7f, history.len());
                feed_bytes(&mut state, b"x", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'k', &history);
                state.process_byte(b'k', &history);
                assert_eq!(state.hist_index, Some(0));
                state.process_byte(b'/', &history);
                for &b in b"zzz" {
                    state.process_byte(b, &history);
                }
                let actions = state.process_byte(b'\r', &history);
                assert!(has_bell(&actions));
            });
        }

        #[test]
        fn vi_search_forward_edit_line_save() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> = vec![
                    b"found"[..].into(),
                    b"skip"[..].into(),
                    b"also_skip"[..].into(),
                ];
                let mut state = ViState::new(0x7f, history.len());
                feed_bytes(&mut state, b"original", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'/', &history);
                for &b in b"found" {
                    state.process_byte(b, &history);
                }
                state.process_byte(b'\r', &history);
                assert_eq!(state.hist_index, Some(0));
                assert_eq!(state.line, b"found");
            });
        }

        #[test]
        fn vi_backslash_glob_nomatch_no_change() {
            assert_no_syscalls(|| {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                let word = b"/no_such_dir_xyzzy/nomatch";
                state.line = word.to_vec();
                state.cursor = state.line.len().saturating_sub(1);
                state.insert_mode = false;
                state.process_byte(b'\\', &history);
                assert_eq!(state.line, word);
            });
        }

        // --- Multi-byte character unit tests ---

        #[test]
        fn word_forward_multibyte() {
            run_trace(trace_entries![], || {
                set_test_locale_utf8();
                assert_eq!(word_forward(b"caf\xc3\xa9 bar", 0), 6);
            });
        }

        #[test]
        fn word_forward_multibyte_punct() {
            run_trace(trace_entries![], || {
                set_test_locale_utf8();
                assert_eq!(word_forward(b"\xc3\xa9.x", 0), 2);
            });
        }

        #[test]
        fn word_backward_multibyte() {
            run_trace(trace_entries![], || {
                set_test_locale_utf8();
                assert_eq!(word_backward(b"ab \xc3\xa9\xc3\xa8", 7), 3);
            });
        }

        #[test]
        fn word_end_multibyte() {
            run_trace(trace_entries![], || {
                set_test_locale_utf8();
                assert_eq!(word_end(b"\xc3\xa9\xc3\xa8 x", 0), 2);
            });
        }

        #[test]
        fn bigword_forward_multibyte() {
            run_trace(trace_entries![], || {
                set_test_locale_utf8();
                assert_eq!(bigword_forward(b"\xc3\xa9.\xc3\xa8 z", 0), 6);
            });
        }

        #[test]
        fn resolve_motion_h_multibyte() {
            run_trace(trace_entries![], || {
                set_test_locale_utf8();
                assert_eq!(resolve_motion(b"a\xc3\xa9b", 3, b'h', 1), (1, 3));
            });
        }

        #[test]
        fn resolve_motion_l_multibyte() {
            run_trace(trace_entries![], || {
                set_test_locale_utf8();
                assert_eq!(resolve_motion(b"a\xc3\xa9b", 1, b'l', 1), (1, 3));
            });
        }

        #[test]
        fn resolve_motion_e_multibyte() {
            run_trace(trace_entries![], || {
                set_test_locale_utf8();
                assert_eq!(resolve_motion(b"\xc3\xa9\xc3\xa8", 0, b'e', 1), (0, 4));
            });
        }

        #[test]
        fn do_find_multibyte_target() {
            run_trace(trace_entries![], || {
                set_test_locale_utf8();
                assert_eq!(do_find(b"a\xc3\xa9b", 0, b'f', 0xe9), Some(1));
            });
        }

        #[test]
        fn do_find_skips_continuation_bytes() {
            run_trace(trace_entries![], || {
                set_test_locale_utf8();
                assert_eq!(do_find(b"a\xc3\xa9b", 0, b'f', b'b' as u32), Some(3));
            });
        }

        #[test]
        fn do_find_t_multibyte() {
            run_trace(trace_entries![], || {
                set_test_locale_utf8();
                assert_eq!(do_find(b"a\xc3\xa9b", 0, b't', b'b' as u32), Some(1));
            });
        }

        #[test]
        fn vi_x_deletes_multibyte_char() {
            run_trace(trace_entries![], || {
                set_test_locale_utf8();
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"a\xc3\xa9b", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'h', &history);
                state.process_byte(b'x', &history);
                assert_eq!(state.line, b"ab");
            });
        }

        #[test]
        fn vi_X_deletes_multibyte_char_before() {
            run_trace(trace_entries![], || {
                set_test_locale_utf8();
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"a\xc3\xa9b", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'X', &history);
                assert_eq!(state.line, b"ab");
            });
        }

        #[test]
        fn vi_dl_multibyte() {
            run_trace(trace_entries![], || {
                set_test_locale_utf8();
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"a\xc3\xa9b", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                state.process_byte(b'l', &history);
                feed_bytes(&mut state, b"dl", &history);
                assert_eq!(state.line, b"ab");
            });
        }

        #[test]
        fn vi_dw_multibyte() {
            run_trace(trace_entries![], || {
                set_test_locale_utf8();
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"\xc3\xa9\xc3\xa8 b", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                feed_bytes(&mut state, b"dw", &history);
                assert_eq!(state.line, b"b");
            });
        }

        #[test]
        fn vi_r_replaces_multibyte() {
            run_trace(trace_entries![], || {
                set_test_locale_utf8();
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"a\xc3\xa9b", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'h', &history);
                feed_bytes(&mut state, b"rX", &history);
                assert_eq!(state.line, b"aXb");
            });
        }

        #[test]
        fn vi_a_appends_after_multibyte() {
            run_trace(trace_entries![], || {
                set_test_locale_utf8();
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"\xc3\xa9b", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                state.process_byte(b'a', &history);
                feed_bytes(&mut state, b"X", &history);
                state.process_byte(0x1b, &history);
                assert_eq!(state.line, b"\xc3\xa9Xb");
            });
        }

        #[test]
        fn vi_dollar_on_multibyte_end() {
            run_trace(trace_entries![], || {
                set_test_locale_utf8();
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"ab\xc3\xa9", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                state.process_byte(b'$', &history);
                assert_eq!(state.cursor, 2);
            });
        }

        #[test]
        fn vi_p_after_multibyte() {
            run_trace(trace_entries![], || {
                set_test_locale_utf8();
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"a\xc3\xa9b", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'h', &history);
                state.process_byte(b'x', &history);
                state.process_byte(b'p', &history);
                assert_eq!(state.line, b"ab\xc3\xa9");
            });
        }

        #[test]
        fn vi_D_multibyte_end() {
            run_trace(trace_entries![], || {
                set_test_locale_utf8();
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"a\xc3\xa9", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'0', &history);
                state.process_byte(b'D', &history);
                assert!(state.line.is_empty());
            });
        }

        #[test]
        fn vi_pipe_column_multibyte() {
            run_trace(trace_entries![], || {
                set_test_locale_utf8();
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"\xc3\xa9\xc3\xa8\xc3\xa0", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'2', &history);
                state.process_byte(b'|', &history);
                assert_eq!(state.cursor, 2);
            });
        }

        #[test]
        fn vi_search_backspace_multibyte() {
            run_trace(trace_entries![], || {
                set_test_locale_utf8();
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                feed_bytes(&mut state, b"a", &history);
                state.process_byte(0x1b, &history);
                state.process_byte(b'/', &history);
                state.process_byte(0xc3, &history);
                state.process_byte(0xa9, &history);
                state.process_byte(0x7f, &history);
                assert!(state.search_buf.is_empty());
            });
        }

        #[test]
        fn last_char_start_multibyte() {
            run_trace(trace_entries![], || {
                set_test_locale_utf8();
                assert_eq!(last_char_start(b"ab\xc3\xa9"), 2);
                assert_eq!(last_char_start(b"\xc3\xa9"), 0);
                assert_eq!(last_char_start(b""), 0);
                assert_eq!(last_char_start(b"a"), 0);
            });
        }

        #[test]
        fn replay_cmd_x_multibyte() {
            run_trace(trace_entries![], || {
                set_test_locale_utf8();
                let mut line = b"a\xc3\xa9b".to_vec();
                let mut cursor = 1usize;
                let mut yank = vec![];
                replay_cmd(&mut line, &mut cursor, &mut yank, b'x', 1, None);
                assert_eq!(line, b"ab");
                assert_eq!(yank, b"\xc3\xa9");
            });
        }

        #[test]
        fn replay_cmd_X_multibyte() {
            run_trace(trace_entries![], || {
                set_test_locale_utf8();
                let mut line = b"a\xc3\xa9b".to_vec();
                let mut cursor = 3usize;
                let mut yank = vec![];
                replay_cmd(&mut line, &mut cursor, &mut yank, b'X', 1, None);
                assert_eq!(line, b"ab");
                assert_eq!(yank, b"\xc3\xa9");
                assert_eq!(cursor, 1);
            });
        }

        #[test]
        fn replay_cmd_r_multibyte() {
            run_trace(trace_entries![], || {
                set_test_locale_utf8();
                let mut line = b"a\xc3\xa9b".to_vec();
                let mut cursor = 1usize;
                let mut yank = vec![];
                replay_cmd(&mut line, &mut cursor, &mut yank, b'r', 1, Some(b'X'));
                assert_eq!(line, b"aXb");
            });
        }
    }

    #[test]
    fn vi_read_line_returns_line_on_enter() {
        run_trace(
            trace_entries![
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                read(fd(STDIN_FILENO), _) -> bytes([b'h']),
                write(fd(STDOUT_FILENO), bytes([b'h'])) -> auto,
                read(fd(STDIN_FILENO), _) -> bytes([b'\n']),
                write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                let result = super::read_line(&mut shell, b"").unwrap();
                assert_eq!(result, Some(b"h\n".to_vec()));
            },
        );
    }

    #[test]
    fn vi_read_line_eof_returns_none() {
        run_trace(
            trace_entries![
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                read(fd(STDIN_FILENO), _) -> bytes(b""),
                write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                let result = super::read_line(&mut shell, b"").unwrap();
                assert_eq!(result, None);
            },
        );
    }

    #[test]
    fn vi_read_line_bell_and_redraw() {
        run_trace(
            trace_entries![
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                read(fd(STDIN_FILENO), _) -> bytes([b'a']),
                write(fd(STDOUT_FILENO), bytes([b'a'])) -> auto,
                read(fd(STDIN_FILENO), _) -> bytes([0x1b]),
                write(fd(STDOUT_FILENO), bytes(b"\x1b[D")) -> auto,
                read(fd(STDIN_FILENO), _) -> bytes([b'Q']),
                write(fd(STDOUT_FILENO), bytes(b"\x07")) -> auto,
                read(fd(STDIN_FILENO), _) -> bytes([b'\r']),
                write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                let result = super::read_line(&mut shell, b"").unwrap();
                assert_eq!(result, Some(b"a\n".to_vec()));
            },
        );
    }

    #[test]
    fn vi_read_line_redraw_on_motion() {
        run_trace(
            trace_entries![
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                read(fd(STDIN_FILENO), _) -> bytes([b'a']),
                write(fd(STDOUT_FILENO), bytes([b'a'])) -> auto,
                read(fd(STDIN_FILENO), _) -> bytes([b'b']),
                write(fd(STDOUT_FILENO), bytes([b'b'])) -> auto,
                read(fd(STDIN_FILENO), _) -> bytes([0x1b]),
                write(fd(STDOUT_FILENO), bytes(b"\x1b[D")) -> auto,
                read(fd(STDIN_FILENO), _) -> bytes([b'h']),
                write(fd(STDOUT_FILENO), bytes(b"\x1b[1D")) -> auto,
                read(fd(STDIN_FILENO), _) -> bytes([b'\r']),
                write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                let result = super::read_line(&mut shell, b"").unwrap();
                assert_eq!(result, Some(b"ab\n".to_vec()));
            },
        );
    }

    #[test]
    fn vi_read_line_eof_with_nonempty_continues() {
        run_trace(
            trace_entries![
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                read(fd(STDIN_FILENO), _) -> bytes([b'x']),
                write(fd(STDOUT_FILENO), bytes([b'x'])) -> auto,
                read(fd(STDIN_FILENO), _) -> bytes(b""),
                read(fd(STDIN_FILENO), _) -> bytes([b'\n']),
                write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                let result = super::read_line(&mut shell, b"").unwrap();
                assert_eq!(result, Some(b"x\n".to_vec()));
            },
        );
    }

    #[test]
    fn vi_read_line_erase_char_fallback() {
        run_trace(
            trace_entries![
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
                tcgetattr(fd(STDIN_FILENO)) -> err(libc::EINVAL),
                read(fd(STDIN_FILENO), _) -> bytes([b'\n']),
                write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                let result = super::read_line(&mut shell, b"").unwrap();
                assert_eq!(result, Some(b"\n".to_vec()));
            },
        );
    }

    #[test]
    fn vi_read_line_tcgetattr_error_falls_back() {
        run_trace(
            trace_entries![
                tcgetattr(fd(STDIN_FILENO)) -> err(libc::ENOTTY),
                read(fd(STDIN_FILENO), _) -> bytes(b""),
            ],
            || {
                let mut shell = test_shell();
                let result = super::read_line(&mut shell, b"").unwrap();
                assert_eq!(result, None);
            },
        );
    }

    #[test]
    fn vi_read_line_redraw_covers_full_redraw() {
        run_trace(
            trace_entries![
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                read(fd(STDIN_FILENO), _) -> bytes([b'a']),
                write(fd(STDOUT_FILENO), bytes([b'a'])) -> auto,
                read(fd(STDIN_FILENO), _) -> bytes([b'b']),
                write(fd(STDOUT_FILENO), bytes([b'b'])) -> auto,
                read(fd(STDIN_FILENO), _) -> bytes([0x1b]),
                write(fd(STDOUT_FILENO), bytes(b"\x1b[D")) -> auto,
                read(fd(STDIN_FILENO), _) -> bytes([b'b']),
                write(fd(STDOUT_FILENO), bytes(b"\r\x1b[K")) -> auto,
                write(fd(STDOUT_FILENO), bytes(b"ab\x1b[2D")) -> auto,
                read(fd(STDIN_FILENO), _) -> bytes([b'\r']),
                write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                let result = super::read_line(&mut shell, b"").unwrap();
                assert_eq!(result, Some(b"ab\n".to_vec()));
            },
        );
    }

    #[test]
    fn vi_read_line_read_error_propagates() {
        run_trace(
            trace_entries![
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                read(fd(STDIN_FILENO), _) -> err(libc::EIO),
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                let result = super::read_line(&mut shell, b"");
                assert!(result.is_err());
            },
        );
    }

    #[test]
    fn vi_read_line_count_digit_triggers_readbyte() {
        run_trace(
            trace_entries![
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                read(fd(STDIN_FILENO), _) -> bytes([b'a']),
                write(fd(STDOUT_FILENO), bytes([b'a'])) -> auto,
                read(fd(STDIN_FILENO), _) -> bytes([0x1b]),
                write(fd(STDOUT_FILENO), bytes(b"\x1b[D")) -> auto,
                read(fd(STDIN_FILENO), _) -> bytes([b'2']),
                read(fd(STDIN_FILENO), _) -> bytes([b'l']),
                write(fd(STDOUT_FILENO), bytes(b"\x07")) -> auto,
                read(fd(STDIN_FILENO), _) -> bytes([b'\r']),
                write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                let result = super::read_line(&mut shell, b"").unwrap();
                assert_eq!(result, Some(b"a\n".to_vec()));
            },
        );
    }

    #[test]
    fn vi_read_line_insert_mode_change() {
        run_trace(
            trace_entries![
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                read(fd(STDIN_FILENO), _) -> bytes([0x1b]),
                read(fd(STDIN_FILENO), _) -> bytes([b'i']),
                read(fd(STDIN_FILENO), _) -> bytes([b'\r']),
                write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                let result = super::read_line(&mut shell, b"").unwrap();
                assert_eq!(result, Some(b"\n".to_vec()));
            },
        );
    }

    #[test]
    fn vi_read_line_find_triggers_need_find_target() {
        run_trace(
            trace_entries![
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                read(fd(STDIN_FILENO), _) -> bytes([b'a']),
                write(fd(STDOUT_FILENO), bytes([b'a'])) -> auto,
                read(fd(STDIN_FILENO), _) -> bytes([0x1b]),
                write(fd(STDOUT_FILENO), bytes(b"\x1b[D")) -> auto,
                read(fd(STDIN_FILENO), _) -> bytes([b'f']),
                read(fd(STDIN_FILENO), _) -> bytes([b'z']),
                write(fd(STDOUT_FILENO), bytes(b"\x07")) -> auto,
                write(fd(STDOUT_FILENO), bytes(b"\r\x1b[K")) -> auto,
                write(fd(STDOUT_FILENO), bytes(b"a\x1b[1D")) -> auto,
                read(fd(STDIN_FILENO), _) -> bytes([b'\r']),
                write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                let result = super::read_line(&mut shell, b"").unwrap();
                assert_eq!(result, Some(b"a\n".to_vec()));
            },
        );
    }

    #[test]
    fn vi_read_line_v_command_empty_file_redraws() {
        run_trace(
            trace_entries![
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                read(fd(STDIN_FILENO), _) -> bytes([0x1b]),
                read(fd(STDIN_FILENO), _) -> bytes([b'v']),
                getpid() -> 42,
                open(_, _, _) -> 10,
                write(fd(10), bytes(b"\n")) -> auto,
                close(fd(10)) -> 0,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
                write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
                open(_, _, _) -> err(libc::ENOENT),
                unlink(_) -> 0,
                write(fd(STDOUT_FILENO), bytes(b"\r\x1b[K")) -> auto,
                read(fd(STDIN_FILENO), _) -> bytes([b'\r']),
                write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                let _ = shell.set_var(b"EDITOR", b":");
                let result = super::read_line(&mut shell, b"").unwrap();
                assert_eq!(result, Some(b"\n".to_vec()));
            },
        );
    }

    #[test]
    fn vi_read_line_v_command_whitespace_only_redraws() {
        run_trace(
            trace_entries![
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                read(fd(STDIN_FILENO), _) -> bytes([0x1b]),
                read(fd(STDIN_FILENO), _) -> bytes([b'v']),
                getpid() -> 42,
                open(_, _, _) -> 10,
                write(fd(10), bytes(b"\n")) -> auto,
                close(fd(10)) -> 0,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
                write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
                open(_, _, _) -> 11,
                read(fd(11), _) -> bytes(b"\n"),
                read(fd(11), _) -> 0,
                close(fd(11)) -> 0,
                unlink(_) -> 0,
                write(fd(STDOUT_FILENO), bytes(b"\r\x1b[K")) -> auto,
                read(fd(STDIN_FILENO), _) -> bytes([b'\r']),
                write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                let _ = shell.set_var(b"EDITOR", b":");
                let result = super::read_line(&mut shell, b"").unwrap();
                assert_eq!(result, Some(b"\n".to_vec()));
            },
        );
    }

    #[test]
    fn vi_read_line_v_command_runs_editor() {
        run_trace(
            trace_entries![
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
                tcgetattr(fd(STDIN_FILENO)) -> 0,
                read(fd(STDIN_FILENO), _) -> bytes([0x1b]),
                read(fd(STDIN_FILENO), _) -> bytes([b'v']),
                getpid() -> 42,
                open(_, _, _) -> 10,
                write(fd(10), bytes(b"\n")) -> auto,
                close(fd(10)) -> 0,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
                write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
                open(_, _, _) -> 11,
                read(fd(11), _) -> bytes(b"edited\n"),
                read(fd(11), _) -> 0,
                close(fd(11)) -> 0,
                unlink(_) -> 0,
                write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                tcsetattr(fd(STDIN_FILENO), int(1)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                let _ = shell.set_var(b"EDITOR", b":");
                let result = super::read_line(&mut shell, b"").unwrap();
                assert_eq!(result, Some(b"edited\n".to_vec()));
            },
        );
    }
}
