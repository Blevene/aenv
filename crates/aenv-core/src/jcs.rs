//! RFC 8785 JSON Canonicalization Scheme (JCS).
//!
//! Deterministic JSON serialization used as a hash-input transformation.
//! Object keys are sorted by UTF-16 code unit ordering, numbers use
//! ECMAScript `JSON.stringify` shortest-form serialization, strings are
//! minimally escaped, and there is no extraneous whitespace.

use serde_json::Value;

/// Canonicalize a `serde_json::Value` to its RFC 8785 representation.
pub fn canonicalize(v: &Value) -> String {
    let mut out = String::new();
    write_value(v, &mut out);
    out
}

fn write_value(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => out.push_str(&format_number(n)),
        Value::String(s) => write_string(s, out),
        Value::Array(xs) => {
            out.push('[');
            for (i, x) in xs.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(x, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            // RFC 8785: keys are sorted by UTF-16 code unit. For ASCII keys
            // (the common case) UTF-16 ordering equals byte ordering. For
            // multi-byte keys we sort by the UTF-16 encoding.
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_by(|a, b| utf16_cmp(a, b));
            out.push('{');
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(k, out);
                out.push(':');
                write_value(&map[*k], out);
            }
            out.push('}');
        }
    }
}

fn utf16_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let mut ai = a.encode_utf16();
    let mut bi = b.encode_utf16();
    loop {
        match (ai.next(), bi.next()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(x), Some(y)) => match x.cmp(&y) {
                std::cmp::Ordering::Equal => continue,
                ord => return ord,
            },
        }
    }
}

fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{09}' => out.push_str("\\t"),
            '\u{0a}' => out.push_str("\\n"),
            '\u{0c}' => out.push_str("\\f"),
            '\u{0d}' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn format_number(n: &serde_json::Number) -> String {
    // Integer fast path (most parameter values are integers).
    if let Some(i) = n.as_i64() {
        return i.to_string();
    }
    if let Some(u) = n.as_u64() {
        return u.to_string();
    }
    let f = n.as_f64().expect("serde_json::Number is i64, u64, or f64");
    format_ecmascript_f64(f)
}

fn format_ecmascript_f64(f: f64) -> String {
    if f == 0.0 {
        return "0".to_string();
    }
    if f.is_nan() || f.is_infinite() {
        return "null".to_string();
    }
    let abs = f.abs();
    if (1e-6..1e21).contains(&abs) {
        format!("{f}")
    } else {
        let s = format!("{f:e}");
        normalize_exponent_sign(&s)
    }
}

fn normalize_exponent_sign(s: &str) -> String {
    if let Some(epos) = s.find('e') {
        let (mantissa, exp) = s.split_at(epos);
        let exp_body = &exp[1..];
        if exp_body.starts_with('-') || exp_body.starts_with('+') {
            format!("{mantissa}e{exp_body}")
        } else {
            format!("{mantissa}e+{exp_body}")
        }
    } else {
        s.to_string()
    }
}
