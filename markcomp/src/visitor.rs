use core::fmt::Debug;
use std::io::Write;
use std::sync::LazyLock;
use wincomp::element::Element;

type VResult<E> = Result<(), E>;

#[derive(Debug)]
pub struct Image<'s> {
    pub alt: &'s [u8],
    pub url: &'s [u8],
    pub title: Option<&'s [u8]>,
}

#[derive(Debug)]
pub struct Code<'s> {
    pub value: &'s [u8],
    pub lang: Option<&'s [u8]>,
}

pub trait Visitor: core::fmt::Debug {
    type Error;

    fn block_quote_enter(&mut self) -> VResult<Self::Error>;
    fn block_quote_exit(&mut self) -> VResult<Self::Error>;

    fn footnote_definition_enter(
        &mut self,
        identifier: &[u8],
        label: Option<&[u8]>,
    ) -> VResult<Self::Error>;
    fn footnote_definition_exit(&mut self) -> VResult<Self::Error>;

    fn footnote_reference(
        &mut self,
        identifier: &[u8],
        label: Option<&[u8]>,
    ) -> VResult<Self::Error>;

    fn yaml(&mut self, yaml: &[u8]) -> VResult<Self::Error>;
    fn page_break(&mut self) -> VResult<Self::Error>;

    fn inline_code(&mut self, code: &[u8]) -> VResult<Self::Error>;
    fn inline_math(&mut self, math: &[u8]) -> VResult<Self::Error>;

    fn delete_enter(&mut self) -> VResult<Self::Error>;
    fn delete_exit(&mut self) -> VResult<Self::Error>;

    fn emphasis_enter(&mut self) -> VResult<Self::Error>;
    fn emphasis_exit(&mut self) -> VResult<Self::Error>;

    fn strong_enter(&mut self) -> VResult<Self::Error>;
    fn strong_exit(&mut self) -> VResult<Self::Error>;

    fn html(&mut self, html: Element<'_>) -> VResult<Self::Error>;

    fn image(&mut self, image: Image<'_>) -> VResult<Self::Error>;

    fn link_enter(&mut self) -> VResult<Self::Error>;
    fn link_exit(&mut self, url: &[u8]) -> VResult<Self::Error>;

    fn text(&mut self, text: &[u8]) -> VResult<Self::Error>;

    fn code(&mut self, code: Code<'_>) -> VResult<Self::Error>;
    fn math(&mut self, math: &[u8]) -> VResult<Self::Error>;

    fn heading_enter(&mut self, level: u8) -> VResult<Self::Error>;
    fn heading_exit(&mut self, level: u8) -> VResult<Self::Error>;

    fn paragraph_enter(&mut self) -> VResult<Self::Error>;
    fn paragraph_exit(&mut self) -> VResult<Self::Error>;
}

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

fn advance_to<F>(mut parser: F, hint: u8) -> impl for<'s> FnMut(&mut &'s [u8]) -> &'s [u8]
where
    F: FnMut(&[u8]) -> bool,
{
    move |input| {
        for (i, byte) in input.iter().enumerate() {
            if *byte == hint && parser(&input[i..]) {
                let result = &input[..i];
                *input = &input[i..];

                return result;
            }
        }

        let result = *input;
        *input = &[];
        result
    }
}

fn starts_with(input: &mut &[u8], sequence: &[u8]) -> bool {
    if input.starts_with(sequence) {
        *input = &input[sequence.len()..];
        true
    } else {
        false
    }
}

fn starts_with_byte(input: &mut &[u8], byte: u8) -> bool {
    if input.first().is_some_and(|f| *f == byte) {
        *input = &input[1..];
        true
    } else {
        false
    }
}

fn skip(input: &mut &[u8], n: usize) {
    *input = &input[input.len().min(n)..];
}

fn skip_newlines<'s>(input: &mut &'s [u8]) -> usize {
    for (i, c) in input.iter().enumerate() {
        if *c != b'\r' && *c != b'\n' {
            *input = &input[i..];
            return i;
        }
    }

    *input = &[];
    input.len()
}

fn parse_strong<'s, V: Visitor>(input: &mut &'s [u8], visitor: &mut V) -> Result<(), V::Error> {
    visitor.strong_enter()?;
    paragraph(|input| input.starts_with(&[b'*']), input, visitor)?;
    skip(input, 1);
    visitor.strong_exit()
}

fn parse_em<'s, V: Visitor>(input: &mut &'s [u8], visitor: &mut V) -> Result<(), V::Error> {
    visitor.strong_enter()?;
    paragraph(|input| input.starts_with(&[b'_']), input, visitor)?;
    skip(input, 1);
    visitor.strong_exit()
}

fn parse_link<'s, V: Visitor>(input: &mut &'s [u8], visitor: &mut V) -> Result<(), V::Error> {
    visitor.link_enter()?;
    paragraph(|input| input.starts_with(&[b']']), input, visitor)?;
    skip(input, 2);

    let url = if starts_with_byte(input, b'<') {
        let url = advance_to(|_| true, b'>')(input);
        advance_to(|_| true, b')')(input);

        url
    } else {
        advance_to(|_| true, b')')(input)
    };

    visitor.link_exit(url)?;
    skip(input, 1);
    Ok(())
}

fn parse_image<'s, V: Visitor>(input: &mut &'s [u8], visitor: &mut V) -> Result<(), V::Error> {
    let alt = advance_to(|_| true, b']')(input);
    skip(input, 2);

    let url = if starts_with_byte(input, b'<') {
        let url = advance_to(|_| true, b'>')(input);
        advance_to(|_| true, b')')(input);

        url
    } else {
        advance_to(|_| true, b')')(input)
    };

    visitor.image(Image {
        alt,
        url,
        title: None,
    })?;
    skip(input, 1);
    Ok(())
}

fn take_while<'s, F>(input: &mut &'s [u8], mut predicate: F) -> &'s [u8]
where
    F: FnMut(u8) -> bool,
{
    let mut stop = input.len();
    for i in 0..input.len() {
        if !predicate(input[i]) {
            stop = i;
            break;
        }
    }

    let output = &input[..stop];
    *input = &input[stop..];

    output
}

fn paragraph<'s, F, V: Visitor>(
    mut terminus: F,
    input: &mut &'s [u8],
    visitor: &mut V,
) -> Result<(), V::Error>
where
    F: FnMut(&[u8]) -> bool,
{
    while !input.is_empty() {
        if terminus(input) {
            return Ok(());
        }

        if starts_with(input, &[b'!', b'[']) {
            parse_image(input, visitor)?;
        } else if starts_with_byte(input, b'*') {
            parse_strong(input, visitor)?;
        } else if starts_with_byte(input, b'_') {
            parse_em(input, visitor)?;
        } else if starts_with(input, "[^".as_bytes()) {
            let ident = advance_to(|_| true, b']')(input);
            visitor.footnote_reference(ident, None)?;
            skip(input, 1);
        } else if starts_with_byte(input, b'[') {
            parse_link(input, visitor)?;
        } else if starts_with_byte(input, b'`') {
            let code = advance_to(|_| true, b'`')(input);
            visitor.inline_code(code)?;
            skip(input, 1);
        } else {
            let mut stop = input.len();
            for (i, c) in input.iter().enumerate().skip(1) {
                if [b'!', b'*', b'_', b'`', b'['].contains(c) || terminus(&input[i..]) {
                    stop = i;
                    break;
                }
            }

            let text = &input[..stop];
            *input = &input[stop..];

            visitor.text(text)?;
        }
    }

    Ok(())
}

fn simple<'s, V: Visitor>(mut input: &'s [u8], visitor: &mut V) -> Result<(), V::Error> {
    let input = &mut input;

    while !input.is_empty() {
        let yaml_seq = "---".as_bytes();
        let code_seq = "```".as_bytes();
        let math_seq = "$$".as_bytes();
        let footnote_seq = "[^".as_bytes();

        if starts_with(input, footnote_seq) {
            let ident = advance_to(|_| true, b']')(input);
            visitor.footnote_definition_enter(ident, None)?;
            skip(input, 2);

            paragraph(
                |input| input.starts_with(&[b'\n']) || input.starts_with(&[b'\r']),
                input,
                visitor,
            )?;
            visitor.footnote_definition_exit();

            skip_newlines(input);
        } else if starts_with(input, yaml_seq) {
            let yaml = advance_to(|i| i.starts_with(yaml_seq), b'-')(input);
            skip(input, yaml_seq.len());

            visitor.yaml(yaml)?;
            skip_newlines(input);
        } else if starts_with(input, code_seq) {
            // parse lang
            let lang = take_while(input, |c| ![b' ', b'\n', b'\r'].contains(&c));
            skip_newlines(input);

            let code = advance_to(|i| i.starts_with(code_seq), b'`')(input);
            skip(input, code_seq.len());

            let code = Code {
                value: code,
                lang: (!lang.is_empty()).then_some(lang),
            };

            visitor.code(code)?;
            skip_newlines(input);
        } else if starts_with(input, math_seq) {
            let math = advance_to(|i| i.starts_with(math_seq), b'$')(input);
            skip(input, math_seq.len());

            visitor.math(math)?;
            skip_newlines(input);
        } else if starts_with(input, &[b'#']) {
            let depth = 1 + take_while(input, |c| c == b'#').len();
            visitor.heading_enter(depth as u8)?;
            paragraph(
                |input| input.starts_with(&[b'\n']) || input.starts_with(&[b'\r']),
                input,
                visitor,
            )?;
            visitor.heading_exit(depth as u8)?;
            skip_newlines(input);
        } else {
            // try to parse as HTML
            if input.starts_with(&[b'<']) {
                let str_input = &mut std::str::from_utf8(input).unwrap();

                if let Ok(element) = wincomp::parse::element(str_input) {
                    visitor.html(element)?;
                    skip_newlines(input);
                    *input = str_input.as_bytes();
                    continue;
                }
            }

            visitor.paragraph_enter()?;
            paragraph(
                |input| {
                    if input.starts_with(&[b'\n']) || input.starts_with(&[b'\r']) {
                        let input = &input[1..];

                        input.starts_with(&[b'\n']) || input.starts_with(&[b'#'])
                    } else {
                        false
                    }
                },
                input,
                visitor,
            )?;
            visitor.paragraph_exit()?;
            skip_newlines(input);
        }
    }

    Ok(())
}

static SET: LazyLock<syntect::parsing::SyntaxSet> =
    LazyLock::new(|| syntect::parsing::SyntaxSet::load_defaults_newlines());

#[derive(Debug, serde::Deserialize)]
pub struct Frontmatter {
    pub title: String,
    pub date: String,
    pub description: String,
}

#[derive(Debug, Clone, Copy)]
enum State {
    Normal,
    Link,
    Footnote,
}

#[derive(Debug)]
pub struct SimpleVisitor {
    state: Vec<State>,
    output: Vec<u8>,
    link_buffer: Vec<u8>,
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

impl SimpleVisitor {
    fn state(&self) -> State {
        self.state.last().copied().unwrap_or(State::Normal)
    }

    fn buffer(&mut self) -> &mut Vec<u8> {
        match self.state() {
            State::Normal => &mut self.output,
            State::Link => &mut self.link_buffer,
            State::Footnote => &mut self.footnotes,
        }
    }

    pub fn new(input: &[u8]) -> Result<Self, SimpleError> {
        let mut visitor = Self {
            frontmatter: None,
            state: vec![State::Normal],
            output: Vec::with_capacity(input.len()),
            link_buffer: Vec::new(),
            footnotes: Vec::new(),
        };

        simple(input, &mut visitor)?;

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

impl Visitor for SimpleVisitor {
    type Error = SimpleError;

    fn yaml(&mut self, yaml: &[u8]) -> VResult<Self::Error> {
        let frontmatter = serde_yaml::from_slice(yaml).map_err(|_| SimpleError)?;
        self.frontmatter = Some(frontmatter);

        Ok(())
    }

    fn block_quote_enter(&mut self) -> VResult<Self::Error> {
        write!(self.buffer(), "<blockquote>").unwrap();
        Ok(())
    }
    fn block_quote_exit(&mut self) -> VResult<Self::Error> {
        write!(self.buffer(), "</blockquote>").unwrap();
        Ok(())
    }

    fn footnote_definition_enter(
        &mut self,
        identifier: &[u8],
        _label: Option<&[u8]>,
    ) -> VResult<Self::Error> {
        self.state.push(State::Footnote);
        let buffer = self.buffer();

        write!(buffer, r#"<p><span id="fn"#).unwrap();
        buffer.extend(identifier);
        write!(buffer, r#"">"#).unwrap();
        buffer.extend(identifier);
        write!(buffer, ".</span>").unwrap();

        write!(buffer, r##"<FootnoteRet href="#ref"##);
        buffer.extend(identifier);
        write!(buffer, r#""/>"#).unwrap();

        Ok(())
    }

    fn footnote_definition_exit(&mut self) -> VResult<Self::Error> {
        write!(self.buffer(), r#"</p>"#).unwrap();
        self.state.pop();

        Ok(())
    }

    fn footnote_reference(
        &mut self,
        identifier: &[u8],
        _label: Option<&[u8]>,
    ) -> VResult<Self::Error> {
        let buffer = self.buffer();

        write!(buffer, r##"<FootnoteRef href="#fn"##);
        buffer.extend(identifier);
        write!(buffer, r#"" id="ref"#);
        buffer.extend(identifier);
        write!(buffer, r#"">"#);
        buffer.extend(identifier);
        write!(buffer, r#"</FootnoteRef>"#);

        Ok(())
    }

    fn page_break(&mut self) -> VResult<Self::Error> {
        Ok(())
    }

    fn inline_code(&mut self, code: &[u8]) -> VResult<Self::Error> {
        let buffer = self.buffer();
        write!(buffer, "<code>").unwrap();
        html_encode(code, buffer).unwrap();
        write!(buffer, "</code>").unwrap();

        Ok(())
    }
    fn inline_math(&mut self, math: &[u8]) -> VResult<Self::Error> {
        let buffer = self.buffer();
        write!(buffer, "<code>").unwrap();
        html_encode(math, buffer).unwrap();
        write!(buffer, "</code>").unwrap();

        Ok(())
    }

    fn delete_enter(&mut self) -> VResult<Self::Error> {
        write!(self.buffer(), "<delete>").unwrap();
        Ok(())
    }
    fn delete_exit(&mut self) -> VResult<Self::Error> {
        write!(self.buffer(), "</delete>").unwrap();
        Ok(())
    }

    fn emphasis_enter(&mut self) -> VResult<Self::Error> {
        write!(self.buffer(), "<em>").unwrap();
        Ok(())
    }
    fn emphasis_exit(&mut self) -> VResult<Self::Error> {
        write!(self.buffer(), "</em>").unwrap();
        Ok(())
    }

    fn strong_enter(&mut self) -> VResult<Self::Error> {
        write!(self.buffer(), "<strong>").unwrap();
        Ok(())
    }
    fn strong_exit(&mut self) -> VResult<Self::Error> {
        write!(self.buffer(), "</strong>").unwrap();
        Ok(())
    }

    fn html(&mut self, html: Element<'_>) -> VResult<Self::Error> {
        html.write(self.buffer()).unwrap();
        Ok(())
    }

    fn image(&mut self, image: Image<'_>) -> VResult<Self::Error> {
        let buffer = self.buffer();

        write!(buffer, r#"<Image src=""#).unwrap();
        buffer.extend(image.url.trim_ascii());
        write!(buffer, r#"" alt=""#).unwrap();
        buffer.extend(image.alt.trim_ascii());
        write!(buffer, r#"" />"#).unwrap();

        Ok(())
    }

    fn link_enter(&mut self) -> VResult<Self::Error> {
        self.state.push(State::Link);
        Ok(())
    }

    fn link_exit(&mut self, url: &[u8]) -> VResult<Self::Error> {
        self.state.pop();
        // let buffer = self.buffer();

        let buffer = match self.state() {
            State::Footnote => &mut self.footnotes,
            _ => &mut self.output,
        };

        write!(buffer, r#"<Link href=""#).unwrap();
        buffer.extend(url.trim_ascii());
        write!(buffer, r#"">"#).unwrap();
        buffer.append(&mut self.link_buffer);
        write!(buffer, "</Link>").unwrap();

        Ok(())
    }

    fn text(&mut self, text: &[u8]) -> VResult<Self::Error> {
        self.buffer().extend(text);
        // self.buffer().push(b' ');
        Ok(())
    }

    fn code(&mut self, code: Code<'_>) -> VResult<Self::Error> {
        if let Some(lang) = code.lang {
            if let Some(syntax) = SET.find_syntax_by_extension(core::str::from_utf8(lang).unwrap())
            {
                write!(self.buffer(), r#"<div class="codeblock">"#).unwrap();

                let theme = include_bytes!("../themes/kanagawa.tmTheme");
                let theme = syntect::highlighting::ThemeSet::load_from_reader(
                    &mut std::io::Cursor::new(theme),
                )
                .unwrap();

                let output = syntect::html::highlighted_html_for_string(
                    core::str::from_utf8(code.value).unwrap(),
                    &SET,
                    syntax,
                    &theme,
                )
                .unwrap();

                write!(self.buffer(), "{}</div>", output).unwrap();

                return Ok(());
            }
        }

        let buffer = self.buffer();
        write!(buffer, "<blockquote>",).unwrap();
        html_encode(code.value, buffer).unwrap();
        write!(buffer, "</blockquote>",).unwrap();

        Ok(())
    }
    fn math(&mut self, math: &[u8]) -> VResult<Self::Error> {
        let buffer = self.buffer();
        write!(buffer, "<blockquote>").unwrap();
        html_encode(math, buffer).unwrap();
        write!(buffer, "</blockquote>").unwrap();

        Ok(())
    }

    fn heading_enter(&mut self, level: u8) -> VResult<Self::Error> {
        write!(self.buffer(), "<h{}>", level).unwrap();
        Ok(())
    }
    fn heading_exit(&mut self, level: u8) -> VResult<Self::Error> {
        write!(self.buffer(), "</h{}>", level).unwrap();
        Ok(())
    }

    fn paragraph_enter(&mut self) -> VResult<Self::Error> {
        write!(self.buffer(), "<p>").unwrap();
        Ok(())
    }
    fn paragraph_exit(&mut self) -> VResult<Self::Error> {
        write!(self.buffer(), "</p>").unwrap();
        Ok(())
    }
}

// #[cfg(test)]
// mod test {
//     use super::*;
//
//     #[test]
//     fn test_simple() {
//         let input = "---\nyaml stuff\n---\nAnd then text stuff.";
//
//         let v = SimpleVisitor::new(input.as_bytes());
//
//         panic!("{}", core::str::from_utf8(&v.output).unwrap());
//     }
//
//     #[test]
//     fn test_small() {
//         let input = std::fs::read_to_string("../test-data/small.md").unwrap();
//
//         let v = SimpleVisitor::new(input.as_bytes());
//
//         panic!("{}", core::str::from_utf8(&v.output).unwrap());
//     }
// }
