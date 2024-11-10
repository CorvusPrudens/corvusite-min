use wincomp::element::Element;
use winnow::{
    ascii::{line_ending, multispace0, space0},
    combinator::{alt, delimited, fail, opt, peek, preceded, repeat, terminated},
    error::{AddContext, ContextError, ErrMode, StrContext},
    stream::{ContainsToken, Stream},
    token::{any, take_until, take_while},
    PResult, Parser,
};

#[derive(Debug)]
pub struct FootnoteDefinition<'s> {
    pub children: Vec<Node<'s>>,
    pub identifier: &'s str,
    pub label: Option<&'s str>,
}

#[derive(Debug)]
pub struct List<'s> {
    pub children: Vec<Node<'s>>,
    pub start: Option<u32>,
    pub spread: bool,
}

#[derive(Debug)]
pub struct FootnoteReference<'s> {
    pub identifier: &'s str,
    pub label: Option<&'s str>,
}

#[derive(Debug)]
pub struct Image<'s> {
    pub alt: &'s str,
    pub url: &'s str,
    pub title: Option<&'s str>,
}

#[derive(Debug)]
pub struct Link<'s> {
    pub children: Vec<Node<'s>>,
    pub url: &'s str,
    pub title: Option<&'s str>,
}

#[derive(Debug)]
pub struct Code<'s> {
    pub value: &'s str,
    pub lang: Option<&'s str>,
    pub meta: Option<&'s str>,
}

#[derive(Debug)]
pub struct Math<'s> {
    pub value: &'s str,
    pub meta: Option<&'s str>,
}

#[derive(Debug)]
pub struct Heading<'s> {
    pub children: Vec<Node<'s>>,
    pub depth: u8,
}

#[derive(Debug)]
pub enum Node<'s> {
    BlockQuote(Vec<Node<'s>>),
    FootnoteDefinition(FootnoteDefinition<'s>),
    List(List<'s>),
    Yaml(&'s str),
    Break,
    InlineCode(&'s str),
    InlineMath(&'s str),
    Delete(Vec<Node<'s>>),
    Emphasis(Vec<Node<'s>>),
    TextExpression(&'s str),
    FootnoteReference(FootnoteReference<'s>),
    Html(Element<'s>),
    Image(Image<'s>),
    Link(Link<'s>),
    Strong(Vec<Node<'s>>),
    Text(&'s str),
    Code(Code<'s>),
    Math(Math<'s>),
    Heading(Heading<'s>),
    ThematicBreak,
    Paragraph(Vec<Node<'s>>),
}

fn html_encode<W: std::io::Write>(input: &str, writer: &mut W) -> std::io::Result<()> {
    for char in input.chars() {
        match char {
            '&' => write!(writer, "&amp;")?,
            '<' => write!(writer, "&lt;")?,
            '>' => write!(writer, "&gt;")?,
            '"' => write!(writer, "&quot;")?,
            '\'' => write!(writer, "&apos;")?,
            c => write!(writer, "{c}")?,
        }
    }

    Ok(())
}

impl<'s> Node<'s> {
    pub fn write<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        match self {
            Self::BlockQuote(children) => {
                write!(writer, "<blockquote>")?;
                for child in children {
                    child.write(writer)?;
                }
                write!(writer, "</blockquote>")?;
            }
            Self::FootnoteDefinition(_) => todo!("footnote"),
            Self::FootnoteReference(_) => todo!("footnote"),
            Self::List(_) => todo!("list"),
            Self::Yaml(_) => {}
            Self::Break => {
                write!(writer, "<br />")?;
            }
            Self::InlineCode(code) => {
                write!(writer, "<code>")?;
                html_encode(code, writer)?;
                write!(writer, "</code>")?;
            }
            Self::Math(Math { value, .. }) => {
                write!(writer, "<blockquote>")?;
                html_encode(value, writer)?;
                write!(writer, "</blockquote>")?;
            }
            Self::InlineMath(math) => {
                write!(writer, "<code>")?;
                html_encode(math, writer)?;
                write!(writer, "</code>")?;
            }
            Self::Delete(children) => {
                write!(writer, "</delete>")?;
                for child in children {
                    child.write(writer)?;
                }
                write!(writer, "</delete>")?;
            }

            Self::Emphasis(children) => {
                write!(writer, "<em>")?;
                for child in children {
                    child.write(writer)?;
                }
                write!(writer, "</em>")?;
            }
            Self::TextExpression(_) => {}
            Self::Html(el) => el.write(writer)?,
            Self::Image(Image { alt, url, title: _ }) => {
                write!(writer, r#"<img href="{url}" alt="{alt}" />"#)?;
            }
            Self::Link(Link {
                children,
                url,
                title: _,
            }) => {
                write!(writer, r#"<a href="{url}">"#)?;
                for child in children {
                    child.write(writer)?;
                }
                write!(writer, "</a>")?;
            }
            Self::Strong(children) => {
                write!(writer, "<strong>")?;
                for child in children {
                    child.write(writer)?;
                }
                write!(writer, "</strong>")?;
            }
            Self::Text(t) => {
                html_encode(t, writer)?;
                write!(writer, " ")?;
            }
            Self::Code(Code {
                value,
                lang,
                meta: _,
            }) => {
                let set = syntect::parsing::SyntaxSet::load_defaults_newlines();

                match lang.and_then(|lang| set.find_syntax_by_extension(lang)) {
                    Some(lang) => {
                        write!(writer, r#"<div class="codeblock">"#)?;

                        let theme = include_bytes!("../themes/kanagawa.tmTheme");
                        let theme = syntect::highlighting::ThemeSet::load_from_reader(
                            &mut std::io::Cursor::new(theme),
                        )
                        .unwrap();

                        let output =
                            syntect::html::highlighted_html_for_string(&value, &set, &lang, &theme)
                                .unwrap();

                        write!(writer, "{}", output)?;
                        write!(writer, "</div>")?;
                    }
                    None => {
                        write!(writer, "<blockquote>{}</blockquote>", value)?;
                    }
                }
            }
            Self::Heading(Heading { children, depth }) => {
                write!(writer, "<h{}>", depth)?;
                for child in children {
                    child.write(writer)?;
                }
                write!(writer, "</h{}>", depth)?;
            }
            Self::ThematicBreak => todo!(),
            Self::Paragraph(children) => {
                write!(writer, "<p>")?;
                for child in children {
                    child.write(writer)?;
                }
                write!(writer, "</p>")?;
            }
        }

        Ok(())
    }
}

fn inline_code<'s>(input: &mut &'s str) -> PResult<&'s str> {
    '`'.parse_next(input)?;
    let value = take_until(0.., '`').parse_next(input)?;
    '`'.parse_next(input)?;

    Ok(value)
}

fn fence<'a>(mut fence: &'a str) -> impl FnMut(&mut &str) -> PResult<()> + 'a {
    move |input| {
        fence.parse_next(input)?;
        preceded(space0, line_ending).parse_next(input)?;
        Ok(())
    }
}

fn yaml<'s>(input: &mut &'s str) -> PResult<&'s str> {
    fence("---").parse_next(input)?;
    let (value, _) = wincomp::parse::advance_to(fence("---"), '-').parse_next(input)?;

    Ok(value)
}

fn math<'s>(input: &mut &'s str) -> PResult<Math<'s>> {
    fence("$$").parse_next(input)?;
    let (value, _) = wincomp::parse::advance_to(fence("$$"), '$').parse_next(input)?;

    Ok(Math { value, meta: None })
}

fn inline_math<'s>(input: &mut &'s str) -> PResult<&'s str> {
    '$'.parse_next(input)?;
    let value = take_until(0.., '$').parse_next(input)?;
    '$'.parse_next(input)?;

    Ok(value)
}

fn code(fe: &str, hint: char) -> impl for<'s> FnMut(&mut &'s str) -> PResult<Code<'s>> + use<'_> {
    move |input| {
        let mut fe1 = fe;
        fe1.parse_next(input)?;
        let lang = opt(preceded(space0, wincomp::parse::identifier)).parse_next(input)?;
        preceded(space0, line_ending).parse_next(input)?;
        let (value, _) = wincomp::parse::advance_to(fence(fe), hint).parse_next(input)?;

        Ok(Code {
            value,
            lang,
            meta: None,
        })
    }
}

// fn code<'s>(input: &mut &'s str) -> PResult<Code<'s>> {
//     "~~~".parse_next(input)?;
//     let lang = opt(preceded(space0, wincomp::parse::identifier)).parse_next(input)?;
//     preceded(space0, line_ending).parse_next(input)?;
//     let (value, _) = wincomp::parse::advance_to(fence("~~~"), '~').parse_next(input)?;
//
//     Ok(Code {
//         value,
//         lang,
//         meta: None,
//     })
// }

fn strikethrough<'s>(input: &mut &'s str) -> PResult<Vec<Node<'s>>> {
    "~~".parse_next(input)?;
    let children = paragraph('~').parse_next(input)?;
    "~~".parse_next(input)?;

    Ok(children)
}

fn image<'s>(input: &mut &'s str) -> PResult<Image<'s>> {
    "![".parse_next(input)?;
    let alt = take_until(0.., ']').parse_next(input)?;
    "](".parse_next(input)?;
    // TODO: this will not catch URLs with parentheses
    let url = take_until(0.., ')').parse_next(input)?;
    ')'.parse_next(input)?;

    Ok(Image {
        alt,
        url,
        title: None,
    })
}

fn heading<'s>(input: &mut &'s str) -> PResult<Heading<'s>> {
    let depth = take_while(1..256, '#').parse_next(input)?.len() as u8;
    let children = paragraph(('\r', '\n')).parse_next(input)?;
    line_ending(input)?;

    Ok(Heading { children, depth })
}

fn top<'s>(input: &mut &'s str) -> PResult<Node<'s>> {
    let result = terminated(
        winnow::combinator::dispatch! {peek(any);
            '-' => yaml.map(Node::Yaml),
            '<' => wincomp::parse::element.map(Node::Html),
            '`' => code("```", '`').map(Node::Code),
            '~' => code("~~~", '~').map(Node::Code),
            '$' => math.map(Node::Math),
            '#' => heading.map(Node::Heading),
            _ => fail::<_, Node, _>,
        },
        multispace0,
    )
    .parse_next(input);

    let node = match result {
        Ok(n) => n,
        Err(ErrMode::Backtrack(_)) => terminated(top_paragraph, multispace0)
            .map(Node::Paragraph)
            .parse_next(input)?,
        Err(e) => return Err(e),
    };

    Ok(node)
}

fn link<'s>(input: &mut &'s str) -> PResult<Link<'s>> {
    let children = delimited('[', paragraph(']'), ']').parse_next(input)?;
    '('.parse_next(input)?;
    // TODO: this will not catch URLs with parentheses
    let url = take_until(0.., ')').parse_next(input)?;
    ')'.parse_next(input)?;

    Ok(Link {
        children,
        url,
        title: None,
    })
}

fn strong<'s>(input: &mut &'s str) -> PResult<Vec<Node<'s>>> {
    // TODO: not quite right since this may trip on something like
    // **strong * stuff**
    delimited("**", paragraph('*'), "**").parse_next(input)
}

fn emphasis<'s>(input: &mut &'s str) -> PResult<Vec<Node<'s>>> {
    delimited('_', paragraph('_'), '_').parse_next(input)
}

fn inline_node<'s>(input: &mut &'s str) -> PResult<Node<'s>> {
    winnow::combinator::dispatch! {peek(any);
        '*' => strong.map(Node::Strong).context(StrContext::Label("strong")),
        '_' => emphasis.map(Node::Emphasis).context(StrContext::Label("emphasis")),
        '[' => link.map(Node::Link).context(StrContext::Label("link")),
        '!' => image.map(Node::Image).context(StrContext::Label("image")),
        '~' => strikethrough.map(Node::Delete).context(StrContext::Label("delete")),
        '$' => inline_math.map(Node::InlineMath).context(StrContext::Label("inline math")),
        '`' => inline_code.map(Node::InlineCode).context(StrContext::Label("inline code")),
        _ => fail::<_, Node, _>,
    }
    .parse_next(input)
}

fn top_paragraph<'s>(input: &mut &'s str) -> PResult<Vec<Node<'s>>> {
    let mut nodes = Vec::new();
    loop {
        let mut p = terminated(paragraph(('\r', '\n')), opt(line_ending)).parse_next(input)?;
        nodes.append(&mut p);

        if peek::<_, _, (), _>(alt(("~~~", "---", "```", "#", "$$")))
            .parse_next(input)
            .is_ok()
            || peek::<_, _, (), _>(line_ending).parse_next(input).is_ok()
            || input.is_empty()
        {
            break;
        }
    }

    Ok(nodes)
}

fn paragraph<C>(termination: C) -> impl for<'s> FnMut(&mut &'s str) -> PResult<Vec<Node<'s>>>
where
    C: ContainsToken<char>,
{
    move |input| {
        let checkpoint = input.checkpoint();
        let mut string = *input;
        let mut nodes = Vec::new();

        let mut iter = string.char_indices();
        loop {
            let Some((i, c)) = iter.next() else {
                if string.len() > 0 {
                    nodes.push(Node::Text(string));
                }
                break;
            };

            if termination.contains_token(c) {
                if i != 0 {
                    nodes.push(Node::Text(&string[..i]));
                    *input = &string[i..];
                }
                break;
            }

            match c {
                '*' | '[' | '!' | '~' | '$' | '`' | '_' => {
                    *input = &string[i..];
                    match inline_node.parse_next(input) {
                        Ok(node) => {
                            if i != 0 {
                                nodes.push(Node::Text(&string[..i]));
                            }
                            nodes.push(node);
                            string = *input;
                            iter = string.char_indices();
                        }
                        Err(e @ winnow::error::ErrMode::Cut(_)) => return Err(e),
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        if nodes.is_empty() {
            Err(ErrMode::Backtrack(ContextError::new().add_context(
                input,
                &checkpoint,
                StrContext::Expected(winnow::error::StrContextValue::Description("text")),
            )))
        } else {
            Ok(nodes)
        }
    }
}

pub fn document<'s>(input: &mut &'s str) -> PResult<Vec<Node<'s>>> {
    preceded(multispace0, repeat(0.., top)).parse_next(input)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_node() {
        let result = inline_node.parse(&mut "`code`").unwrap();

        assert!(matches!(result, Node::InlineCode(c) if c == "code"));
    }

    #[test]
    fn test_doc() {
        let mut input = "
# Hello, world!

How are `you` doing?


[Here's a link!](wikipedia.com)

<Text>
    Here's some html!
</Text>
";

        let result = document.parse(&mut input);

        // panic!("{result:#?}");

        match result {
            Ok(r) => assert_eq!(r.len(), 3),
            Err(e) => {
                panic!("{e}");
            }
        }
    }
}
