use core::fmt::Debug;
use pulldown_cmark::{CodeBlockKind, Event, MetadataBlockKind, Options, Parser, Tag, TagEnd};
use std::io::Write;
use std::sync::LazyLock;
use syntect::parsing::SyntaxReference;

fn html_encode<W: std::io::Write>(input: &[u8], writer: &mut W) -> std::io::Result<()> {
    for char in input.iter().copied() {
        match char {
            b'&' => write!(writer, "&amp;")?,
            b'<' => write!(writer, "&lt;")?,
            b'>' => write!(writer, "&gt;")?,
            b'"' => write!(writer, "&quot;")?,
            b'\'' => write!(writer, "&apos;")?,
            c => {
                writer.write(&[c])?;
            }
        }
    }

    Ok(())
}

static SET: LazyLock<syntect::parsing::SyntaxSet> =
    LazyLock::new(|| syntect::parsing::SyntaxSet::load_defaults_newlines());

static THEME: LazyLock<syntect::highlighting::Theme> = LazyLock::new(|| {
    let theme = include_bytes!("../themes/kanagawa.tmTheme");
    syntect::highlighting::ThemeSet::load_from_reader(&mut std::io::Cursor::new(theme))
        .expect("Code theme should be valid")
});

#[derive(Debug, serde::Deserialize)]
pub struct Frontmatter {
    pub title: String,
    pub date: String,
    pub description: String,
}

#[derive(Debug, Clone, Copy)]
enum State {
    Normal,
    Footnote,
}

enum Code<'a> {
    Named {
        lang: &'a SyntaxReference,
        code: String,
    },
    Unnamed,
    Html,
    Yaml(Vec<u8>),
}

#[derive(Debug)]
pub struct Writer {
    state: State,
    output: Vec<u8>,
    footnotes: Vec<u8>,
    pub frontmatter: Option<Frontmatter>,
}

/// Indicates malformed YAML.
#[derive(Debug)]
pub struct SimpleError;

impl std::fmt::Display for SimpleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error processing YAML frontmatter")
    }
}

impl std::error::Error for SimpleError {}

impl Writer {
    fn buffer(&mut self) -> &mut Vec<u8> {
        match self.state {
            State::Normal => &mut self.output,
            State::Footnote => &mut self.footnotes,
        }
    }

    fn append(&mut self, string: &str) {
        self.buffer().extend(string.as_bytes());
    }

    fn parse(&mut self, input: &str) -> Result<(), SimpleError> {
        let parser = Parser::new_ext(
            input,
            Options::ENABLE_STRIKETHROUGH
                | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
                | Options::ENABLE_FOOTNOTES
                | Options::ENABLE_MATH,
        );

        let mut code = None;
        let mut footnote_def = None;

        for event in parser {
            match event {
                Event::Start(tag) => match tag {
                    Tag::MetadataBlock(kind) => {
                        if matches!(kind, MetadataBlockKind::YamlStyle) {
                            code = Some(Code::Yaml(Vec::new()));
                        }
                    }
                    Tag::Paragraph => self.append("<p>"),
                    Tag::Emphasis => self.append("<em>"),
                    Tag::Strong => self.append("<strong>"),
                    Tag::Strikethrough => self.append("<delete>"),
                    Tag::Link { dest_url, .. } => {
                        write!(self.buffer(), r#"<Link href="{dest_url}">"#).unwrap();
                    }
                    Tag::Heading { level, .. } => {
                        write!(self.buffer(), r#"<{level}>"#).unwrap();
                    }
                    Tag::FootnoteDefinition(label) => {
                        self.state = State::Footnote;
                        write!(self.buffer(), r#"<li id="fn{label}">"#).unwrap();
                        footnote_def = Some(label);
                    }
                    Tag::CodeBlock(kind) => match kind {
                        CodeBlockKind::Fenced(lang) if !lang.is_empty() => {
                            if let Some(syntax) = SET.find_syntax_by_extension(&lang) {
                                code = Some(Code::Named {
                                    lang: syntax,
                                    code: String::new(),
                                });
                            } else {
                                code = Some(Code::Unnamed);
                                self.append("<blockquote>");
                            }
                        }
                        _ => {
                            code = Some(Code::Unnamed);
                            self.append("<blockquote>");
                        }
                    },
                    Tag::HtmlBlock => code = Some(Code::Html),
                    _ => {} // tag => todo!("tag start: {tag:#?}"),
                },
                Event::End(tag) => match tag {
                    TagEnd::MetadataBlock(kind) => match (kind, code.take()) {
                        (MetadataBlockKind::YamlStyle, Some(Code::Yaml(yaml))) => {
                            let frontmatter =
                                serde_yaml::from_slice(&yaml).map_err(|_| SimpleError)?;
                            self.frontmatter = Some(frontmatter);
                        }
                        _ => {}
                    },
                    TagEnd::Paragraph => self.append("</p>"),
                    TagEnd::Emphasis => self.append("</em>"),
                    TagEnd::Strong => self.append("</strong>"),
                    TagEnd::Strikethrough => self.append("</delete>"),
                    TagEnd::Link => self.append("</Link>"),
                    TagEnd::Heading(level) => write!(self.buffer(), "</{level}>").unwrap(),
                    TagEnd::CodeBlock => match code.take() {
                        Some(Code::Named { lang, code }) => {
                            write!(self.buffer(), r#"<div class="codeblock">"#).unwrap();

                            let output = syntect::html::highlighted_html_for_string(
                                &code, &SET, lang, &THEME,
                            )
                            .unwrap();

                            write!(self.buffer(), "{}</div>", output).unwrap();
                        }
                        Some(Code::Unnamed) => {
                            self.append("</blockquote>");
                        }
                        _ => {}
                    },
                    TagEnd::FootnoteDefinition => {
                        let def = footnote_def.take();
                        let label: &str = def.as_ref().map(|s| s.as_ref()).unwrap_or("?");

                        write!(
                            self.buffer(),
                            r##"<FootnoteRet href="#ref{label}" /></li>"##
                        )
                        .unwrap();
                        self.state = State::Normal;
                    }
                    TagEnd::HtmlBlock => code = None,
                    _ => {} // tag => todo!("tag end: {tag:#?}"),
                },
                Event::Text(t) => match &mut code {
                    Some(Code::Named { code, .. }) => code.push_str(&t),
                    Some(Code::Yaml(yaml)) => yaml.extend(t.as_bytes()),
                    Some(Code::Html) => self.buffer().extend(t.as_bytes()),
                    _ => html_encode(t.as_bytes(), self.buffer()).unwrap(),
                },
                Event::FootnoteReference(label) => {
                    write!(
                        self.buffer(),
                        r##"<FootnoteRef href="#fn{label}" id="ref{label}">{label}</FootnoteRef>"##
                    )
                    .unwrap();
                }
                Event::Html(html) => self.append(&html),
                Event::Code(code) => write!(self.buffer(), "<code>{code}</code>").unwrap(),
                Event::InlineMath(math) => write!(self.buffer(), "<code>{math}</code>").unwrap(),
                Event::SoftBreak => write!(self.buffer(), "\n").unwrap(),
                Event::DisplayMath(math) => {
                    write!(self.buffer(), "<blockquote>{math}</blockquote>").unwrap()
                }
                _ => {} // event => todo!("event: {event:#?}"),
            }
        }

        Ok(())
    }

    pub fn new(input: &str) -> Result<Self, SimpleError> {
        let mut visitor = Self {
            state: State::Normal,
            frontmatter: None,
            output: Vec::with_capacity(input.len()),
            footnotes: Vec::new(),
        };

        visitor.parse(input)?;

        Ok(visitor)
    }

    pub fn output(mut self) -> Vec<u8> {
        if !self.footnotes.is_empty() {
            write!(&mut self.output, "<Footnotes>").unwrap();
            self.output.append(&mut self.footnotes);
            write!(&mut self.output, "</Footnotes>").unwrap();
        }

        self.output
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_codeblock() {
        let input = "~~~rs\nfn hello() {}\n~~~";

        let writer = Writer::new(input).unwrap();
        let _output = writer.output();
    }
}
