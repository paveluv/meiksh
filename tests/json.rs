//! Minimal JSON parser — just enough for requirements.json.
//!
//! Supports: objects, arrays, strings, booleans, null.
//! Does NOT support: numbers (not needed for our use case).

#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Str(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

impl JsonValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            JsonValue::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            JsonValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[JsonValue]> {
        match self {
            JsonValue::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            JsonValue::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn is_null(&self) -> bool {
        matches!(self, JsonValue::Null)
    }
}

struct Parser<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            pos: 0,
        }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.input.len() {
            match self.input[self.pos] {
                b' ' | b'\t' | b'\n' | b'\r' => self.pos += 1,
                _ => break,
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn advance(&mut self) -> Result<u8, String> {
        if self.pos >= self.input.len() {
            return Err("unexpected end of input".into());
        }
        let b = self.input[self.pos];
        self.pos += 1;
        Ok(b)
    }

    fn expect(&mut self, ch: u8) -> Result<(), String> {
        let b = self.advance()?;
        if b != ch {
            Err(format!(
                "expected {:?}, got {:?} at byte {}",
                ch as char,
                b as char,
                self.pos - 1
            ))
        } else {
            Ok(())
        }
    }

    fn parse_value(&mut self) -> Result<JsonValue, String> {
        self.skip_ws();
        match self.peek() {
            Some(b'"') => self.parse_string().map(JsonValue::Str),
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b't') => self.parse_literal(b"true", JsonValue::Bool(true)),
            Some(b'f') => self.parse_literal(b"false", JsonValue::Bool(false)),
            Some(b'n') => self.parse_literal(b"null", JsonValue::Null),
            Some(ch) => Err(format!("unexpected {:?} at byte {}", ch as char, self.pos)),
            None => Err("unexpected end of input".into()),
        }
    }

    fn parse_literal(&mut self, expected: &[u8], value: JsonValue) -> Result<JsonValue, String> {
        for &b in expected {
            let got = self.advance()?;
            if got != b {
                return Err(format!(
                    "expected {:?}, got {:?} at byte {}",
                    b as char,
                    got as char,
                    self.pos - 1
                ));
            }
        }
        Ok(value)
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut buf = Vec::new();
        loop {
            let b = self.advance()?;
            match b {
                b'"' => break,
                b'\\' => {
                    let esc = self.advance()?;
                    match esc {
                        b'"' => buf.push(b'"'),
                        b'\\' => buf.push(b'\\'),
                        b'/' => buf.push(b'/'),
                        b'n' => buf.push(b'\n'),
                        b'r' => buf.push(b'\r'),
                        b't' => buf.push(b'\t'),
                        b'u' => {
                            let mut hex = [0u8; 4];
                            for h in &mut hex {
                                *h = self.advance()?;
                            }
                            let s = std::str::from_utf8(&hex)
                                .map_err(|_| "invalid \\u escape".to_string())?;
                            let cp = u32::from_str_radix(s, 16)
                                .map_err(|_| format!("invalid \\u escape: {s}"))?;
                            if let Some(ch) = char::from_u32(cp) {
                                let mut tmp = [0u8; 4];
                                buf.extend_from_slice(ch.encode_utf8(&mut tmp).as_bytes());
                            }
                        }
                        _ => {
                            buf.push(b'\\');
                            buf.push(esc);
                        }
                    }
                }
                _ => buf.push(b),
            }
        }
        String::from_utf8(buf).map_err(|e| format!("invalid utf-8 in string: {e}"))
    }

    fn parse_array(&mut self) -> Result<JsonValue, String> {
        self.expect(b'[')?;
        self.skip_ws();
        let mut items = Vec::new();
        if self.peek() == Some(b']') {
            self.advance()?;
            return Ok(JsonValue::Array(items));
        }
        loop {
            items.push(self.parse_value()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.advance()?;
                }
                Some(b']') => {
                    self.advance()?;
                    break;
                }
                _ => return Err(format!("expected ',' or ']' at byte {}", self.pos)),
            }
        }
        Ok(JsonValue::Array(items))
    }

    fn parse_object(&mut self) -> Result<JsonValue, String> {
        self.expect(b'{')?;
        self.skip_ws();
        let mut pairs = Vec::new();
        if self.peek() == Some(b'}') {
            self.advance()?;
            return Ok(JsonValue::Object(pairs));
        }
        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.skip_ws();
            self.expect(b':')?;
            let val = self.parse_value()?;
            pairs.push((key, val));
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.advance()?;
                }
                Some(b'}') => {
                    self.advance()?;
                    break;
                }
                _ => return Err(format!("expected ',' or '}}' at byte {}", self.pos)),
            }
        }
        Ok(JsonValue::Object(pairs))
    }
}

pub fn parse_json(input: &str) -> Result<JsonValue, String> {
    let mut p = Parser::new(input);
    let val = p.parse_value()?;
    p.skip_ws();
    if p.pos != p.input.len() {
        return Err(format!("trailing data at byte {}", p.pos));
    }
    Ok(val)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_bool() {
        assert_eq!(parse_json("null").unwrap(), JsonValue::Null);
        assert_eq!(parse_json("true").unwrap(), JsonValue::Bool(true));
        assert_eq!(parse_json("false").unwrap(), JsonValue::Bool(false));
    }

    #[test]
    fn simple_string() {
        assert_eq!(
            parse_json(r#""hello""#).unwrap(),
            JsonValue::Str("hello".into())
        );
    }

    #[test]
    fn string_escapes() {
        assert_eq!(
            parse_json(r#""a\"b\\c""#).unwrap(),
            JsonValue::Str("a\"b\\c".into())
        );
    }

    #[test]
    fn empty_array() {
        assert_eq!(parse_json("[]").unwrap(), JsonValue::Array(vec![]));
    }

    #[test]
    fn array_of_strings() {
        let val = parse_json(r#"["a", "b"]"#).unwrap();
        let arr = val.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0].as_str().unwrap(), "a");
    }

    #[test]
    fn simple_object() {
        let val = parse_json(r#"{"key": "val", "flag": true}"#).unwrap();
        assert_eq!(val.get("key").unwrap().as_str().unwrap(), "val");
        assert_eq!(val.get("flag").unwrap().as_bool().unwrap(), true);
    }

    #[test]
    fn nested() {
        let val = parse_json(r#"[{"id": "X", "ok": null}]"#).unwrap();
        let arr = val.as_array().unwrap();
        assert_eq!(arr[0].get("id").unwrap().as_str().unwrap(), "X");
        assert!(arr[0].get("ok").unwrap().is_null());
    }

    #[test]
    fn trailing_data_err() {
        assert!(parse_json("null null").is_err());
    }
}
