//! RFC 8785 JCS standard test vectors.
//!
//! Vectors drawn from RFC 8785 §3.2.3 ("Object structure") and §3.2.2.3
//! ("Number serialization"). The full RFC test suite includes ECMAScript
//! number-formatting edge cases (1e+30, 1e-7, etc.); we cover the
//! representative cases that exercise our serializer's branches.

use aenv_core::jcs::canonicalize;
use serde_json::json;

#[test]
fn object_keys_are_sorted() {
    let v = json!({"b": 1, "a": 2});
    assert_eq!(canonicalize(&v), r#"{"a":2,"b":1}"#);
}

#[test]
fn nested_object_keys_are_sorted_recursively() {
    let v = json!({"b": 1, "a": {"d": 4, "c": 3}});
    assert_eq!(canonicalize(&v), r#"{"a":{"c":3,"d":4},"b":1}"#);
}

#[test]
fn array_order_is_preserved() {
    let v = json!([3, 1, 2]);
    assert_eq!(canonicalize(&v), "[3,1,2]");
}

#[test]
fn empty_collections() {
    assert_eq!(canonicalize(&json!([])), "[]");
    assert_eq!(canonicalize(&json!({})), "{}");
}

#[test]
fn null_and_booleans() {
    assert_eq!(canonicalize(&json!(null)), "null");
    assert_eq!(canonicalize(&json!(true)), "true");
    assert_eq!(canonicalize(&json!(false)), "false");
}

#[test]
fn integer_numbers() {
    assert_eq!(canonicalize(&json!(0)), "0");
    assert_eq!(canonicalize(&json!(1)), "1");
    assert_eq!(canonicalize(&json!(-1)), "-1");
    assert_eq!(canonicalize(&json!(42)), "42");
    assert_eq!(canonicalize(&json!(i64::MAX)), "9223372036854775807");
}

#[test]
fn float_numbers_use_shortest_form() {
    // ECMAScript JSON.stringify(1.5) -> "1.5"
    assert_eq!(canonicalize(&json!(1.5)), "1.5");
    // 5e1 round-trips to 50 per ECMAScript.
    let v: serde_json::Value = serde_json::from_str("5e1").unwrap();
    assert_eq!(canonicalize(&v), "50");
    // 1e21 stays in exponent form per ECMAScript.
    let v: serde_json::Value = serde_json::from_str("1e21").unwrap();
    assert_eq!(canonicalize(&v), "1e+21");
}

#[test]
fn string_escaping_is_minimal() {
    // Only the RFC 8259 mandatory escapes: quote, backslash, control chars.
    assert_eq!(canonicalize(&json!("hello")), r#""hello""#);
    assert_eq!(canonicalize(&json!("a\"b")), r#""a\"b""#);
    assert_eq!(canonicalize(&json!("a\\b")), r#""a\\b""#);
    assert_eq!(canonicalize(&json!("a\nb")), r#""a\nb""#);
    assert_eq!(canonicalize(&json!("a\tb")), r#""a\tb""#);
    // Non-ASCII Unicode is emitted directly (UTF-8), NOT \uXXXX escaped.
    assert_eq!(canonicalize(&json!("ö")), "\"ö\"");
    assert_eq!(canonicalize(&json!("中")), "\"中\"");
}

#[test]
fn control_chars_below_0x20_get_lowercase_hex_escapes() {
    // RFC 8785 specifies \u escapes for control chars use lowercase hex.
    // Specifically \u00XX form. We test 0x01 and 0x1f.
    let v = json!("\u{01}");
    assert_eq!(canonicalize(&v), "\"\\u0001\"");
    let v = json!("\u{1f}");
    assert_eq!(canonicalize(&v), "\"\\u001f\"");
}

#[test]
fn rfc_8785_section_3_example() {
    // From RFC 8785 §3.2.3 example, simplified:
    let v = json!({
        "numbers": [333333333.3333333, 1e30, 4.50, 0.000001, "10e+0"],
        "string": "Hello world!",
        "literals": [null, true, false]
    });
    let out = canonicalize(&v);
    let expected = r#"{"literals":[null,true,false],"numbers":[333333333.3333333,1e+30,4.5,0.000001,"10e+0"],"string":"Hello world!"}"#;
    assert_eq!(out, expected);
}
