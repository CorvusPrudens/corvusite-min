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
                | Options::ENABLE_MATH
                | Options::ENABLE_STRIKETHROUGH,
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
        let output = writer.output();

        panic!("{}", core::str::from_utf8(&output).unwrap());
    }
}

// impl Visitor for SimpleVisitor {
//     type Error = SimpleError;
//
//     fn yaml(&mut self, yaml: &[u8]) -> VResult<Self::Error> {
//         let frontmatter = serde_yaml::from_slice(yaml).map_err(|_| SimpleError)?;
//         self.frontmatter = Some(frontmatter);
//
//         Ok(())
//     }
//
//     fn block_quote_enter(&mut self) -> VResult<Self::Error> {
//         write!(self.buffer(), "<blockquote>").unwrap();
//         Ok(())
//     }
//     fn block_quote_exit(&mut self) -> VResult<Self::Error> {
//         write!(self.buffer(), "</blockquote>").unwrap();
//         Ok(())
//     }
//
//     fn footnote_definition_enter(
//         &mut self,
//         identifier: &[u8],
//         _label: Option<&[u8]>,
//     ) -> VResult<Self::Error> {
//         self.state.push(State::Footnote);
//         let buffer = self.buffer();
//
//         write!(buffer, r#"<p><span id="fn"#).unwrap();
//         buffer.extend(identifier);
//         write!(buffer, r#"">"#).unwrap();
//         buffer.extend(identifier);
//         write!(buffer, ".</span>").unwrap();
//
//         Ok(())
//     }
//
//     fn footnote_definition_exit(&mut self, identifier: &[u8]) -> VResult<Self::Error> {
//         let buffer = self.buffer();
//
//         write!(buffer, r##"<FootnoteRet href="#ref"##);
//         buffer.extend(identifier);
//         write!(buffer, r#""/>"#).unwrap();
//
//         write!(buffer, r#"</p>"#).unwrap();
//         self.state.pop();
//
//         Ok(())
//     }
//
//     fn footnote_reference(
//         &mut self,
//         identifier: &[u8],
//         _label: Option<&[u8]>,
//     ) -> VResult<Self::Error> {
//         let buffer = self.buffer();
//
//         write!(buffer, r##"<FootnoteRef href="#fn"##);
//         buffer.extend(identifier);
//         write!(buffer, r#"" id="ref"#);
//         buffer.extend(identifier);
//         write!(buffer, r#"">"#);
//         buffer.extend(identifier);
//         write!(buffer, r#"</FootnoteRef>"#);
//
//         Ok(())
//     }
//
//     fn page_break(&mut self) -> VResult<Self::Error> {
//         Ok(())
//     }
//
//     fn inline_code(&mut self, code: &[u8]) -> VResult<Self::Error> {
//         let buffer = self.buffer();
//         write!(buffer, "<code>").unwrap();
//         html_encode(code, buffer).unwrap();
//         write!(buffer, "</code>").unwrap();
//
//         Ok(())
//     }
//     fn inline_math(&mut self, math: &[u8]) -> VResult<Self::Error> {
//         let buffer = self.buffer();
//         write!(buffer, "<code>").unwrap();
//         html_encode(math, buffer).unwrap();
//         write!(buffer, "</code>").unwrap();
//
//         Ok(())
//     }
//
//     fn delete_enter(&mut self) -> VResult<Self::Error> {
//         write!(self.buffer(), "<delete>").unwrap();
//         Ok(())
//     }
//     fn delete_exit(&mut self) -> VResult<Self::Error> {
//         write!(self.buffer(), "</delete>").unwrap();
//         Ok(())
//     }
//
//     fn emphasis_enter(&mut self) -> VResult<Self::Error> {
//         write!(self.buffer(), "<em>").unwrap();
//         Ok(())
//     }
//     fn emphasis_exit(&mut self) -> VResult<Self::Error> {
//         write!(self.buffer(), "</em>").unwrap();
//         Ok(())
//     }
//
//     fn strong_enter(&mut self) -> VResult<Self::Error> {
//         write!(self.buffer(), "<strong>").unwrap();
//         Ok(())
//     }
//     fn strong_exit(&mut self) -> VResult<Self::Error> {
//         write!(self.buffer(), "</strong>").unwrap();
//         Ok(())
//     }
//
//     fn html(&mut self, html: Element<'_>) -> VResult<Self::Error> {
//         html.write(self.buffer()).unwrap();
//         Ok(())
//     }
//
//     fn image(&mut self, image: Image<'_>) -> VResult<Self::Error> {
//         let buffer = self.buffer();
//
//         write!(buffer, r#"<Image src=""#).unwrap();
//         buffer.extend(image.url.trim_ascii());
//         write!(buffer, r#"" alt=""#).unwrap();
//         buffer.extend(image.alt.trim_ascii());
//         write!(buffer, r#"" />"#).unwrap();
//
//         Ok(())
//     }
//
//     fn link_enter(&mut self) -> VResult<Self::Error> {
//         self.state.push(State::Link);
//         Ok(())
//     }
//
//     fn link_exit(&mut self, url: &[u8]) -> VResult<Self::Error> {
//         self.state.pop();
//         // let buffer = self.buffer();
//
//         let buffer = match self.state() {
//             State::Footnote => &mut self.footnotes,
//             _ => &mut self.output,
//         };
//
//         write!(buffer, r#"<Link href=""#).unwrap();
//         buffer.extend(url.trim_ascii());
//         write!(buffer, r#"">"#).unwrap();
//         buffer.append(&mut self.link_buffer);
//         write!(buffer, "</Link>").unwrap();
//
//         Ok(())
//     }
//
//     fn text(&mut self, text: &[u8]) -> VResult<Self::Error> {
//         self.buffer().extend(text);
//         // self.buffer().push(b' ');
//         Ok(())
//     }
//
//     fn code(&mut self, code: Code<'_>) -> VResult<Self::Error> {
//         if let Some(lang) = code.lang {
//             if let Some(syntax) = SET.find_syntax_by_extension(core::str::from_utf8(lang).unwrap())
//             {
//                 write!(self.buffer(), r#"<div class="codeblock">"#).unwrap();
//
//                 let theme = include_bytes!("../themes/kanagawa.tmTheme");
//                 let theme = syntect::highlighting::ThemeSet::load_from_reader(
//                     &mut std::io::Cursor::new(theme),
//                 )
//                 .unwrap();
//
//                 let output = syntect::html::highlighted_html_for_string(
//                     core::str::from_utf8(code.value).unwrap(),
//                     &SET,
//                     syntax,
//                     &theme,
//                 )
//                 .unwrap();
//
//                 write!(self.buffer(), "{}</div>", output).unwrap();
//
//                 return Ok(());
//             }
//         }
//
//         let buffer = self.buffer();
//         write!(buffer, "<blockquote>",).unwrap();
//         html_encode(code.value, buffer).unwrap();
//         write!(buffer, "</blockquote>",).unwrap();
//
//         Ok(())
//     }
//     fn math(&mut self, math: &[u8]) -> VResult<Self::Error> {
//         let buffer = self.buffer();
//         write!(buffer, "<blockquote>").unwrap();
//         html_encode(math, buffer).unwrap();
//         write!(buffer, "</blockquote>").unwrap();
//
//         Ok(())
//     }
//
//     fn heading_enter(&mut self, level: u8) -> VResult<Self::Error> {
//         write!(self.buffer(), "<h{}>", level).unwrap();
//         Ok(())
//     }
//     fn heading_exit(&mut self, level: u8) -> VResult<Self::Error> {
//         write!(self.buffer(), "</h{}>", level).unwrap();
//         Ok(())
//     }
//
//     fn paragraph_enter(&mut self) -> VResult<Self::Error> {
//         write!(self.buffer(), "<p>").unwrap();
//         Ok(())
//     }
//     fn paragraph_exit(&mut self) -> VResult<Self::Error> {
//         write!(self.buffer(), "</p>").unwrap();
//         Ok(())
//     }
// }
//
