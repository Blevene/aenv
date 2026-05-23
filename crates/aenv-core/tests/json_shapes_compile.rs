use aenv_core::json::{
    AdapterEntryJson, DriftReport, GetReport, ListEntry, SkillEntry, StatusReport, StructuralDiff,
    WhichReport,
};

macro_rules! assert_object {
    ($t:ty) => {{
        let v = serde_json::to_value(<$t>::default()).unwrap();
        assert!(
            v.is_object(),
            "{} must serialize as a JSON object",
            stringify!($t)
        );
    }};
}

#[test]
fn every_shape_is_an_object() {
    assert_object!(StatusReport);
    assert_object!(ListEntry);
    assert_object!(WhichReport);
    assert_object!(GetReport);
    assert_object!(AdapterEntryJson);
    assert_object!(SkillEntry);
    assert_object!(DriftReport);
    assert_object!(StructuralDiff);
}
