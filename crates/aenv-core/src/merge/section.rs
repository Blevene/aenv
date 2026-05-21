//! Markdown section-merge.
//!
//! Groups content by Markdown heading. Same-heading bodies across the chain
//! concatenate in chain order by default; an `<!-- aenv:replace -->` marker
//! on the first non-whitespace line after a heading in a later namespace
//! discards earlier bodies for that heading.
//!
//! Headings are matched by their literal text (trimmed). Different heading
//! depths with the same text are distinct sections.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

const REPLACE_MARKER: &str = "<!-- aenv:replace -->";

#[derive(Debug, Clone)]
struct ParsedInput {
    preamble: String,
    sections: Vec<Section>,
}

#[derive(Debug, Clone)]
struct Section {
    key: SectionKey,
    body: String,
    replace: bool,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
struct SectionKey {
    depth: HeadingDepth,
    title: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum HeadingDepth {
    H1,
    H2,
    Other(u8),
}

impl HeadingDepth {
    fn from(level: HeadingLevel) -> Self {
        match level {
            HeadingLevel::H1 => HeadingDepth::H1,
            HeadingLevel::H2 => HeadingDepth::H2,
            HeadingLevel::H3 => HeadingDepth::Other(3),
            HeadingLevel::H4 => HeadingDepth::Other(4),
            HeadingLevel::H5 => HeadingDepth::Other(5),
            HeadingLevel::H6 => HeadingDepth::Other(6),
        }
    }
    fn marker(self) -> &'static str {
        match self {
            HeadingDepth::H1 => "#",
            HeadingDepth::H2 => "##",
            HeadingDepth::Other(3) => "###",
            HeadingDepth::Other(4) => "####",
            HeadingDepth::Other(5) => "#####",
            HeadingDepth::Other(6) => "######",
            _ => "##",
        }
    }
}

/// Merge one or more Markdown inputs in chain order (root first).
///
/// Sections with the same heading key are appended in chain order by default.
/// If a section body opens with `<!-- aenv:replace -->`, all earlier bodies
/// for that heading are discarded and only that section (and later ones) are
/// kept.
pub fn merge_sections(inputs: &[String]) -> String {
    if inputs.is_empty() {
        return String::new();
    }
    let parsed: Vec<ParsedInput> = inputs.iter().map(|s| parse(s)).collect();

    let mut out = String::new();
    for p in &parsed {
        if !p.preamble.trim().is_empty() {
            out.push_str(p.preamble.trim_end());
            out.push_str("\n\n");
        }
    }

    let mut order: Vec<SectionKey> = Vec::new();
    let mut by_key: std::collections::HashMap<SectionKey, Vec<&Section>> = Default::default();
    for p in &parsed {
        for s in &p.sections {
            if !by_key.contains_key(&s.key) {
                order.push(s.key.clone());
            }
            by_key.entry(s.key.clone()).or_default().push(s);
        }
    }

    for key in order {
        let sections = &by_key[&key];
        let start_idx = sections.iter().rposition(|s| s.replace).unwrap_or(0);
        let effective = &sections[start_idx..];

        out.push_str(key.depth.marker());
        out.push(' ');
        out.push_str(&key.title);
        out.push('\n');

        for s in effective {
            let body = s.body.trim_end();
            if body.is_empty() {
                continue;
            }
            out.push('\n');
            out.push_str(body);
            out.push('\n');
        }
        out.push('\n');
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}

fn parse(input: &str) -> ParsedInput {
    let parser = Parser::new_ext(input, Options::empty()).into_offset_iter();
    let mut headings: Vec<(SectionKey, std::ops::Range<usize>)> = Vec::new();
    let mut current_heading: Option<(SectionKey, usize, usize)> = None;

    for (event, range) in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current_heading = Some((
                    SectionKey {
                        depth: HeadingDepth::from(level),
                        title: String::new(),
                    },
                    range.start,
                    range.end,
                ));
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some((ref mut k, _, _)) = current_heading {
                    k.title.push_str(&t);
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((key, start, end)) = current_heading.take() {
                    headings.push((
                        SectionKey {
                            depth: key.depth,
                            title: key.title.trim().to_string(),
                        },
                        start..end,
                    ));
                }
            }
            _ => {}
        }
    }

    let preamble_end = headings.first().map(|(_, r)| r.start).unwrap_or(input.len());
    let preamble = input[..preamble_end].to_string();
    let mut sections = Vec::with_capacity(headings.len());
    for (i, (key, range)) in headings.iter().enumerate() {
        let body_start = range.end;
        let body_end = headings
            .get(i + 1)
            .map(|(_, r2)| r2.start)
            .unwrap_or(input.len());
        let raw_body = &input[body_start..body_end];

        let trimmed = raw_body.trim_start_matches(['\n', ' ']);
        let (replace, body) = if let Some(rest) = trimmed.strip_prefix(REPLACE_MARKER) {
            let after = rest.strip_prefix('\n').unwrap_or(rest).to_string();
            (true, after)
        } else {
            (false, raw_body.trim_start_matches('\n').to_string())
        };

        sections.push(Section {
            key: key.clone(),
            body,
            replace,
        });
    }
    ParsedInput { preamble, sections }
}
