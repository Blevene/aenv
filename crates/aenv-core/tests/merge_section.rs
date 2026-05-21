use aenv_core::merge::section::merge_sections;

#[test]
fn empty_inputs_produce_empty_output() {
    let out = merge_sections(&[]);
    assert_eq!(out, "");
}

#[test]
fn single_input_passes_through_unchanged() {
    let body = "# Top\n\nsome text\n";
    let out = merge_sections(&[body.to_string()]);
    assert_eq!(out, body);
}

#[test]
fn distinct_top_sections_concatenate_in_chain_order() {
    let base = "# Build & Test\n\ncargo test\n";
    let leaf = "# Disposition\n\nbe terse\n";
    let out = merge_sections(&[base.to_string(), leaf.to_string()]);
    assert!(out.starts_with("# Build & Test"));
    assert!(out.contains("# Disposition"));
    assert!(out.contains("cargo test"));
    assert!(out.contains("be terse"));
}

#[test]
fn same_section_appends_by_default() {
    let base = "## Conventions\n\n- single quotes\n";
    let leaf = "## Conventions\n\n- four-space indent\n";
    let out = merge_sections(&[base.to_string(), leaf.to_string()]);
    let heading_count = out.matches("## Conventions").count();
    assert_eq!(heading_count, 1, "should de-duplicate the heading");
    let single = out.find("- single quotes").unwrap();
    let four = out.find("- four-space indent").unwrap();
    assert!(single < four, "base's content precedes leaf's");
}

#[test]
fn replace_marker_overrides_append() {
    let base = "## Conventions\n\n- single quotes\n";
    let leaf = "## Conventions\n<!-- aenv:replace -->\n\n- four-space indent\n";
    let out = merge_sections(&[base.to_string(), leaf.to_string()]);
    assert!(!out.contains("single quotes"));
    assert!(out.contains("four-space indent"));
    assert!(!out.contains("aenv:replace"));
}

#[test]
fn preamble_before_first_heading_is_preserved_per_namespace() {
    let base = "Some preamble.\n\n# Top\n\nbody\n";
    let leaf = "# Top\n\nleaf body\n";
    let out = merge_sections(&[base.to_string(), leaf.to_string()]);
    assert!(out.starts_with("Some preamble.\n\n"));
    assert!(out.contains("body"));
    assert!(out.contains("leaf body"));
}

#[test]
fn nested_subsections_merge_under_their_parent_heading() {
    let base = "## Build\n\n### Lint\n\nclippy\n";
    let leaf = "## Build\n\n### Test\n\ncargo test\n";
    let out = merge_sections(&[base.to_string(), leaf.to_string()]);
    let build_count = out.matches("## Build").count();
    assert_eq!(build_count, 1);
    assert!(out.contains("### Lint"));
    assert!(out.contains("### Test"));
}

#[test]
fn three_level_chain_appends_in_order() {
    let a = "## X\n\na\n";
    let b = "## X\n\nb\n";
    let c = "## X\n\nc\n";
    let out = merge_sections(&[a.to_string(), b.to_string(), c.to_string()]);
    let ia = out.find("\na\n").unwrap();
    let ib = out.find("\nb\n").unwrap();
    let ic = out.find("\nc\n").unwrap();
    assert!(ia < ib && ib < ic);
}

#[test]
fn replace_in_middle_of_chain_replaces_only_prior_content() {
    let a = "## X\n\na\n";
    let b = "## X\n<!-- aenv:replace -->\n\nb\n";
    let c = "## X\n\nc\n";
    let out = merge_sections(&[a.to_string(), b.to_string(), c.to_string()]);
    assert!(!out.contains("\na\n"));
    let ib = out.find("\nb\n").unwrap();
    let ic = out.find("\nc\n").unwrap();
    assert!(ib < ic);
}
