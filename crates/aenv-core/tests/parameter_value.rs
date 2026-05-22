use aenv_core::parameters::ParameterValue;

#[test]
fn parses_string() {
    let pv: ParameterValue = ParameterValue::from_toml_value(&toml::Value::String(
        "claude-opus-4.7".to_owned(),
    ))
    .unwrap();
    assert_eq!(pv, ParameterValue::String("claude-opus-4.7".into()));
}

#[test]
fn parses_integer() {
    let pv = ParameterValue::from_toml_value(&toml::Value::Integer(3000)).unwrap();
    assert_eq!(pv, ParameterValue::Integer(3000));
}

#[test]
fn parses_boolean() {
    let pv = ParameterValue::from_toml_value(&toml::Value::Boolean(true)).unwrap();
    assert_eq!(pv, ParameterValue::Boolean(true));
}

#[test]
fn parses_list_of_strings() {
    let arr = toml::Value::Array(vec![
        toml::Value::String("code-reviewer".into()),
        toml::Value::String("write-tests".into()),
    ]);
    let pv = ParameterValue::from_toml_value(&arr).unwrap();
    assert_eq!(
        pv,
        ParameterValue::ListString(vec!["code-reviewer".into(), "write-tests".into()])
    );
}

#[test]
fn rejects_float() {
    let err = ParameterValue::from_toml_value(&toml::Value::Float(1.5)).unwrap_err();
    assert!(err.to_string().contains("float"));
}

#[test]
fn rejects_datetime() {
    let dt = toml::Value::Datetime("1979-05-27T07:32:00Z".parse().unwrap());
    let err = ParameterValue::from_toml_value(&dt).unwrap_err();
    assert!(err.to_string().contains("datetime"));
}

#[test]
fn rejects_inline_table() {
    let mut t = toml::value::Table::new();
    t.insert("k".into(), toml::Value::String("v".into()));
    let err = ParameterValue::from_toml_value(&toml::Value::Table(t)).unwrap_err();
    assert!(err.to_string().contains("table"));
}

#[test]
fn rejects_mixed_array() {
    let arr = toml::Value::Array(vec![
        toml::Value::String("ok".into()),
        toml::Value::Integer(7),
    ]);
    let err = ParameterValue::from_toml_value(&arr).unwrap_err();
    assert!(err.to_string().contains("list"));
}

#[test]
fn type_tag_strings() {
    assert_eq!(ParameterValue::String("x".into()).type_tag(), "string");
    assert_eq!(ParameterValue::Integer(0).type_tag(), "integer");
    assert_eq!(ParameterValue::Boolean(false).type_tag(), "boolean");
    assert_eq!(
        ParameterValue::ListString(vec![]).type_tag(),
        "list-of-string"
    );
}

#[test]
fn display_is_human_readable() {
    assert_eq!(format!("{}", ParameterValue::String("a".into())), "a");
    assert_eq!(format!("{}", ParameterValue::Integer(42)), "42");
    assert_eq!(format!("{}", ParameterValue::Boolean(true)), "true");
    assert_eq!(
        format!(
            "{}",
            ParameterValue::ListString(vec!["a".into(), "b".into()])
        ),
        "[\"a\", \"b\"]"
    );
}
