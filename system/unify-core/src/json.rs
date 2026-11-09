//! Small streaming JSON writer.
//!
//! UnifyOS services speak JSON to the desktop applications over stdout and via
//! a few runtime files. Rather than pull a dependency into a system component,
//! the writer lives here: it produces valid JSON, escapes control characters
//! correctly, and can pretty-print for debugging.

use std::fmt::Write as _;

/// Container type of a frame on the writer stack.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Object,
    Array,
}

struct Frame {
    kind: Kind,
    first: bool,
}

/// Streaming JSON writer. Containers are opened and closed explicitly; object
/// fields are written as key/value pairs, array elements as bare values.
pub struct JsonWriter {
    out: String,
    pretty: bool,
    stack: Vec<Frame>,
}

impl JsonWriter {
    /// Writer that emits the most compact representation.
    pub fn compact() -> Self {
        Self {
            out: String::new(),
            pretty: false,
            stack: Vec::new(),
        }
    }

    /// Writer that emits indented, human-readable output.
    pub fn pretty() -> Self {
        Self {
            out: String::new(),
            pretty: true,
            stack: Vec::new(),
        }
    }

    fn newline(&mut self) {
        if self.pretty {
            self.out.push('\n');
            for _ in 0..self.stack.len() {
                self.out.push_str("  ");
            }
        }
    }

    /// Emit separator, indentation and, when present, the key.
    fn begin(&mut self, key: Option<&str>) {
        let need_comma = match self.stack.last_mut() {
            Some(frame) => {
                let was_first = frame.first;
                frame.first = false;
                !was_first
            }
            None => false,
        };
        if need_comma {
            self.out.push(',');
        }
        if !self.stack.is_empty() {
            self.newline();
        }
        if let Some(key) = key {
            push_escaped(&mut self.out, key);
            self.out.push(':');
            if self.pretty {
                self.out.push(' ');
            }
        }
    }

    fn close(&mut self) {
        let frame = self
            .stack
            .pop()
            .expect("JSON container closed without being opened");
        if self.pretty && !frame.first {
            self.newline();
        }
        self.out.push(match frame.kind {
            Kind::Object => '}',
            Kind::Array => ']',
        });
    }

    pub fn begin_object(&mut self, key: Option<&str>) {
        self.begin(key);
        self.out.push('{');
        self.stack.push(Frame {
            kind: Kind::Object,
            first: true,
        });
    }

    pub fn end_object(&mut self) {
        self.close();
    }

    pub fn begin_array(&mut self, key: Option<&str>) {
        self.begin(key);
        self.out.push('[');
        self.stack.push(Frame {
            kind: Kind::Array,
            first: true,
        });
    }

    pub fn end_array(&mut self) {
        self.close();
    }

    pub fn str_field(&mut self, key: &str, value: &str) {
        self.begin(Some(key));
        push_escaped(&mut self.out, value);
    }

    pub fn str_value(&mut self, value: &str) {
        self.begin(None);
        push_escaped(&mut self.out, value);
    }

    pub fn int_field(&mut self, key: &str, value: i64) {
        self.begin(Some(key));
        let _ = write!(self.out, "{value}");
    }

    pub fn int_value(&mut self, value: i64) {
        self.begin(None);
        let _ = write!(self.out, "{value}");
    }

    pub fn uint_field(&mut self, key: &str, value: u64) {
        self.begin(Some(key));
        let _ = write!(self.out, "{value}");
    }

    pub fn uint_value(&mut self, value: u64) {
        self.begin(None);
        let _ = write!(self.out, "{value}");
    }

    pub fn num_field(&mut self, key: &str, value: f64) {
        self.begin(Some(key));
        self.number(value);
    }

    pub fn num_value(&mut self, value: f64) {
        self.begin(None);
        self.number(value);
    }

    pub fn bool_field(&mut self, key: &str, value: bool) {
        self.begin(Some(key));
        self.out.push_str(if value { "true" } else { "false" });
    }

    pub fn bool_value(&mut self, value: bool) {
        self.begin(None);
        self.out.push_str(if value { "true" } else { "false" });
    }

    pub fn null_field(&mut self, key: &str) {
        self.begin(Some(key));
        self.out.push_str("null");
    }

    pub fn null_value(&mut self) {
        self.begin(None);
        self.out.push_str("null");
    }

    fn number(&mut self, value: f64) {
        if !value.is_finite() {
            self.out.push_str("null");
            return;
        }
        if value.fract() == 0.0 && value.abs() < 9.007_199_254_740_992e15 {
            let _ = write!(self.out, "{}", value as i64);
            return;
        }
        let mut formatted = format!("{value:.4}");
        while formatted.ends_with('0') {
            formatted.pop();
        }
        if formatted.ends_with('.') {
            formatted.pop();
        }
        self.out.push_str(&formatted);
    }

    /// Consume the writer and return the encoded document.
    pub fn finish(self) -> String {
        debug_assert!(
            self.stack.is_empty(),
            "unbalanced JSON containers when finishing"
        );
        self.out
    }
}

fn push_escaped(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::JsonWriter;

    #[test]
    fn compact_document() {
        let mut w = JsonWriter::compact();
        w.begin_object(None);
        w.str_field("name", "UnifyOS");
        w.uint_field("cores", 2);
        w.bool_field("ok", true);
        w.null_field("missing");
        w.end_object();
        assert_eq!(
            w.finish(),
            r#"{"name":"UnifyOS","cores":2,"ok":true,"missing":null}"#
        );
    }

    #[test]
    fn arrays_and_objects_nest() {
        let mut w = JsonWriter::compact();
        w.begin_object(None);
        w.begin_array(Some("cpu"));
        w.num_value(12.5);
        w.num_value(0.0);
        w.end_array();
        w.begin_object(Some("mem"));
        w.uint_field("total", 1024);
        w.end_object();
        w.end_object();
        assert_eq!(
            w.finish(),
            r#"{"cpu":[12.5,0],"mem":{"total":1024}}"#
        );
    }

    #[test]
    fn escaping_is_valid_json() {
        let mut w = JsonWriter::compact();
        w.begin_object(None);
        w.str_field("path", "a\"b\\c\nd\te\u{1}");
        w.end_object();
        assert_eq!(
            w.finish(),
            r#"{"path":"a\"b\\c\nd\te\u0001"}"#
        );
    }

    #[test]
    fn pretty_document_layout() {
        let mut w = JsonWriter::pretty();
        w.begin_object(None);
        w.str_field("name", "UnifyOS");
        w.begin_array(Some("cores"));
        w.num_value(1.0);
        w.num_value(2.5);
        w.end_array();
        w.end_object();
        let expected = "{\n  \"name\": \"UnifyOS\",\n  \"cores\": [\n    1,\n    2.5\n  ]\n}";
        assert_eq!(w.finish(), expected);
    }

    #[test]
    fn non_finite_numbers_become_null() {
        let mut w = JsonWriter::compact();
        w.begin_array(None);
        w.num_value(f64::NAN);
        w.num_value(f64::INFINITY);
        w.end_array();
        assert_eq!(w.finish(), "[null,null]");
    }
}
