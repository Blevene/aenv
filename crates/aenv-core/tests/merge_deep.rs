use aenv_core::merge::deep_json::merge_json;

#[test]
fn merges_two_objects_union_of_keys() {
    let a = br#"{"a":1,"b":2}"#;
    let b = br#"{"b":20,"c":3}"#;
    let out = merge_json(&[a.to_vec(), b.to_vec()]).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["a"], 1);
    assert_eq!(v["b"], 20);
    assert_eq!(v["c"], 3);
}

#[test]
fn arrays_concatenate_in_chain_order() {
    let a = br#"{"x":[1,2]}"#;
    let b = br#"{"x":[3]}"#;
    let out = merge_json(&[a.to_vec(), b.to_vec()]).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["x"].as_array().unwrap().len(), 3);
    assert_eq!(v["x"][0], 1);
    assert_eq!(v["x"][2], 3);
}

#[test]
fn nested_objects_merge_recursively() {
    let a = br#"{"servers":{"a":{"command":"cmd-a"}}}"#;
    let b = br#"{"servers":{"b":{"command":"cmd-b"}}}"#;
    let out = merge_json(&[a.to_vec(), b.to_vec()]).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert!(v["servers"]["a"]["command"] == "cmd-a");
    assert!(v["servers"]["b"]["command"] == "cmd-b");
}

#[test]
fn type_mismatch_later_wins() {
    let a = br#"{"x":1}"#;
    let b = br#"{"x":"string"}"#;
    let out = merge_json(&[a.to_vec(), b.to_vec()]).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["x"], "string");
}

#[test]
fn null_loses_to_value() {
    let a = br#"{"x":null}"#;
    let b = br#"{"x":1}"#;
    let out = merge_json(&[a.to_vec(), b.to_vec()]).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["x"], 1);
}

#[test]
fn invalid_json_returns_parse_error() {
    let a = br#"{"x":"#; // truncated
    let err = merge_json(&[a.to_vec()]).unwrap_err();
    assert!(matches!(
        err,
        aenv_core::merge::MergeError::Parse { kind: "json", .. }
    ));
}

#[test]
fn three_way_chain_preserves_order() {
    let a = br#"{"list":[1]}"#;
    let b = br#"{"list":[2]}"#;
    let c = br#"{"list":[3]}"#;
    let out = merge_json(&[a.to_vec(), b.to_vec(), c.to_vec()]).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let arr = v["list"].as_array().unwrap();
    assert_eq!(arr.len(), 3);
    assert_eq!(arr[0], 1);
    assert_eq!(arr[1], 2);
    assert_eq!(arr[2], 3);
}

#[test]
fn output_is_stable_two_space_pretty() {
    let a = br#"{"a":1}"#;
    let b = br#"{"b":2}"#;
    let out = merge_json(&[a.to_vec(), b.to_vec()]).unwrap();
    let text = std::str::from_utf8(&out).unwrap();
    assert!(text.starts_with("{\n  \""));
}

use aenv_core::merge::deep_yaml::merge_yaml;

#[test]
fn yaml_merges_objects_union_of_keys() {
    let a = b"a: 1\nb: 2\n";
    let b = b"b: 20\nc: 3\n";
    let out = merge_yaml(&[a.to_vec(), b.to_vec()]).unwrap();
    let v: serde_yaml::Value = serde_yaml::from_slice(&out).unwrap();
    assert_eq!(v["a"], serde_yaml::Value::Number(1.into()));
    assert_eq!(v["b"], serde_yaml::Value::Number(20.into()));
    assert_eq!(v["c"], serde_yaml::Value::Number(3.into()));
}

#[test]
fn yaml_arrays_concatenate() {
    let a = b"list:\n  - 1\n  - 2\n";
    let b = b"list:\n  - 3\n";
    let out = merge_yaml(&[a.to_vec(), b.to_vec()]).unwrap();
    let v: serde_yaml::Value = serde_yaml::from_slice(&out).unwrap();
    let arr = v["list"].as_sequence().unwrap();
    assert_eq!(arr.len(), 3);
}

#[test]
fn yaml_nested_objects_merge_recursively() {
    let a = b"servers:\n  a:\n    cmd: cmd-a\n";
    let b = b"servers:\n  b:\n    cmd: cmd-b\n";
    let out = merge_yaml(&[a.to_vec(), b.to_vec()]).unwrap();
    let v: serde_yaml::Value = serde_yaml::from_slice(&out).unwrap();
    assert_eq!(
        v["servers"]["a"]["cmd"],
        serde_yaml::Value::String("cmd-a".into())
    );
    assert_eq!(
        v["servers"]["b"]["cmd"],
        serde_yaml::Value::String("cmd-b".into())
    );
}

#[test]
fn yaml_invalid_input_returns_parse_error() {
    let a = b"[unclosed\n"; // definitely invalid
    let err = merge_yaml(&[a.to_vec()]).unwrap_err();
    assert!(matches!(
        err,
        aenv_core::merge::MergeError::Parse { kind: "yaml", .. }
    ));
}

use aenv_core::merge::deep_toml::merge_toml;

#[test]
fn toml_merges_tables_union_of_keys() {
    let a = b"a = 1\nb = 2\n";
    let b = b"b = 20\nc = 3\n";
    let out = merge_toml(&[a.to_vec(), b.to_vec()]).unwrap();
    let v: toml::Value = toml::from_str(std::str::from_utf8(&out).unwrap()).unwrap();
    assert_eq!(v["a"].as_integer().unwrap(), 1);
    assert_eq!(v["b"].as_integer().unwrap(), 20);
    assert_eq!(v["c"].as_integer().unwrap(), 3);
}

#[test]
fn toml_arrays_concatenate() {
    let a = b"list = [1, 2]\n";
    let b = b"list = [3]\n";
    let out = merge_toml(&[a.to_vec(), b.to_vec()]).unwrap();
    let v: toml::Value = toml::from_str(std::str::from_utf8(&out).unwrap()).unwrap();
    assert_eq!(v["list"].as_array().unwrap().len(), 3);
}

#[test]
fn toml_nested_tables_merge_recursively() {
    let a = b"[adapters.cc]\nfiles = [\"a\"]\n";
    let b = b"[adapters.cursor]\nfiles = [\"b\"]\n";
    let out = merge_toml(&[a.to_vec(), b.to_vec()]).unwrap();
    let v: toml::Value = toml::from_str(std::str::from_utf8(&out).unwrap()).unwrap();
    assert!(v["adapters"]["cc"]["files"].is_array());
    assert!(v["adapters"]["cursor"]["files"].is_array());
}

#[test]
fn toml_invalid_input_returns_parse_error() {
    let a = b"= invalid\n";
    let err = merge_toml(&[a.to_vec()]).unwrap_err();
    assert!(matches!(
        err,
        aenv_core::merge::MergeError::Parse { kind: "toml", .. }
    ));
}
