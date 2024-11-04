use wincomp::element::Element;
use winnow::{
    ascii::{line_ending, multispace0, space0},
    combinator::{delimited, fail, opt, peek, preceded, repeat, terminated},
    error::{AddContext, ContextError, ErrMode, ParseError, StrContext, StrContextValue},
    stream::{Accumulate, ContainsToken, Stream},
    token::{any, take_until, take_while},
    PResult, Parser, Stateful,
};

#[derive(Debug)]
pub struct NodeArena<'s>(Vec<Node<'s>>);

/// The same size as a Vec<NodeId>, but with
/// enough space to fit seven IDs on the stack.
#[derive(Debug)]
pub struct NodeVec(tinyvec::TinyVec<[NodeId; 7]>);

impl NodeVec {
    pub fn children<'s, 'b>(
        &self,
        nodes: &'b NodeArena<'s>,
    ) -> impl Iterator<Item = &'b Node<'s>> + use<'s, 'b, '_> {
        self.0.iter().copied().map(|n| &nodes[n])
    }

    pub fn ids(&self) -> impl Iterator<Item = NodeId> + use<'_> {
        self.0.iter().copied()
    }
}

impl Accumulate<NodeId> for NodeVec {
    fn initial(capacity: Option<usize>) -> Self {
        match capacity {
            Some(cap) => {
                let vec = tinyvec::TinyVec::with_capacity(cap);
                NodeVec(vec)
            }
            None => NodeVec(tinyvec::TinyVec::new()),
        }
    }

    fn accumulate(&mut self, acc: NodeId) {
        self.0.push(acc)
    }
}

type Input<'s, 'b> = Stateful<&'s str, &'b mut NodeArena<'s>>;

impl<'s> NodeArena<'s> {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn insert(&mut self, node: Node<'s>) -> NodeId {
        let id = self.0.len();
        self.0.push(node);
        NodeId(id as u16)
    }
}

impl<'s> std::ops::Index<NodeId> for NodeArena<'s> {
    type Output = Node<'s>;

    fn index(&self, index: NodeId) -> &Self::Output {
        &self.0[index.0 as usize]
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NodeId(u16);

#[derive(Debug)]
pub struct FootnoteDefinition<'s> {
    pub children: NodeVec,
    pub identifier: &'s str,
    pub label: Option<&'s str>,
}

#[derive(Debug)]
pub struct List {
    pub children: NodeVec,
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
    pub children: NodeVec,
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
pub struct Heading {
    pub children: NodeVec,
    pub depth: u8,
}

#[derive(Debug)]
pub enum Node<'s> {
    BlockQuote(NodeVec),
    FootnoteDefinition(FootnoteDefinition<'s>),
    List(List),
    Yaml(&'s str),
    Break,
    InlineCode(&'s str),
    InlineMath(&'s str),
    Delete(NodeVec),
    Emphasis(NodeVec),
    TextExpression(&'s str),
    FootnoteReference(FootnoteReference<'s>),
    Html(Element<'s>),
    Image(Image<'s>),
    Link(Link<'s>),
    Strong(NodeVec),
    Text(&'s str),
    Code(Code<'s>),
    Math(Math<'s>),
    Heading(Heading),
    ThematicBreak,
    Paragraph(NodeVec),
}

impl<'s> Node<'s> {
    pub fn write<W: std::io::Write>(
        &self,
        writer: &mut W,
        arena: &NodeArena<'s>,
    ) -> std::io::Result<()> {
        match self {
            Self::BlockQuote(children) => {
                write!(writer, "<blockquote>")?;
                for child in children.children(arena) {
                    child.write(writer, arena)?;
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
                write!(writer, "<code>{code}</code>")?;
            }
            Self::Math(Math { value, .. }) => {
                write!(writer, "<blockquote>{value}</blockquote>")?;
            }
            Self::InlineMath(math) => {
                write!(writer, "<code>{math}</code>")?;
            }
            Self::Delete(children) => {
                write!(writer, "</delete>")?;
                for child in children.children(arena) {
                    child.write(writer, arena)?;
                }
                write!(writer, "</delete>")?;
            }

            Self::Emphasis(children) => {
                write!(writer, "<em>")?;
                for child in children.children(arena) {
                    child.write(writer, arena)?;
                }
                write!(writer, "</em>")?;
            }
            Self::TextExpression(_) => {}
            Self::Html(el) => el.write(writer)?,
            Self::Image(Image { alt, url, title }) => {
                write!(writer, r#"<img href="{url}" alt="{alt}" />"#)?;
            }
            Self::Link(Link {
                children,
                url,
                title,
            }) => {
                write!(writer, r#"<a href="{url}">"#)?;
                for child in children.0.iter() {
                    arena[*child].write(writer, arena)?;
                }
                write!(writer, "</a>")?;
            }
            Self::Strong(children) => {
                write!(writer, "<strong>")?;
                for child in children.children(arena) {
                    child.write(writer, arena)?;
                }
                write!(writer, "</strong>")?;
            }
            Self::Text(t) => {
                write!(writer, "<p>{t}</p>")?;
            }
            Self::Code(Code { value, lang, meta }) => {
                write!(writer, "<blockquote>{value}</blockquote>")?;
            }
            Self::Heading(Heading { children, depth }) => {
                write!(writer, "<h{}>", depth)?;
                for child in children.children(arena) {
                    child.write(writer, arena)?;
                }
                write!(writer, "</h{}>", depth)?;
            }
            Self::ThematicBreak => todo!(),
            Self::Paragraph(children) => {
                write!(writer, "<p>")?;
                for child in children.children(arena) {
                    child.write(writer, arena)?;
                }
                write!(writer, "</p>")?;
            }
        }

        Ok(())
    }
}

fn inline_code<'s, 'b>(input: &mut Input<'s, 'b>) -> PResult<&'s str> {
    '`'.parse_next(input)?;
    let value = take_until(0.., '`').parse_next(input)?;
    '`'.parse_next(input)?;

    Ok(value)
}

fn fence<'a>(mut fence: &'a str) -> impl for<'s, 'b> FnMut(&mut Input<'s, 'b>) -> PResult<()> + 'a {
    move |input| {
        fence.parse_next(input)?;
        preceded(space0, line_ending).parse_next(input)?;
        Ok(())
    }
}

pub fn advance_to<P, O, E>(
    mut parser: P,
    hint: char,
) -> impl for<'s, 'b> FnMut(&mut Input<'s, 'b>) -> PResult<(&'s str, O)>
where
    P: for<'s, 'b> Parser<Input<'s, 'b>, O, E>,
{
    move |input| {
        let start = input.input;
        let checkpoint = input.checkpoint();

        for (i, c) in input.input.char_indices() {
            if c == hint {
                let old_input = input.input;
                input.input = &old_input[i..];

                if let Ok(p) = parser.parse_next(input) {
                    return Ok((&start[..i], p));
                } else {
                    input.input = old_input;
                }
            }
        }

        Err(ErrMode::Cut(ContextError::default().add_context(
            input,
            &checkpoint,
            StrContext::Expected(StrContextValue::Description("ending pattern")),
        )))
    }
}

fn yaml<'s, 'b>(input: &mut Input<'s, 'b>) -> PResult<&'s str> {
    fence("---").parse_next(input)?;
    let (value, _) = advance_to(fence("---"), '-').parse_next(input)?;

    Ok(value)
}

fn math<'s, 'b>(input: &mut Input<'s, 'b>) -> PResult<Math<'s>> {
    fence("$$").parse_next(input)?;
    let (value, _) = advance_to(fence("$$"), '$').parse_next(input)?;

    Ok(Math { value, meta: None })
}

fn inline_math<'s, 'b>(input: &mut Input<'s, 'b>) -> PResult<&'s str> {
    '$'.parse_next(input)?;
    let value = take_until(0.., '$').parse_next(input)?;
    '$'.parse_next(input)?;

    Ok(value)
}

pub fn map_identifier<'s, 'b>(input: &mut Input<'s, 'b>) -> PResult<&'s str> {
    wincomp::parse::identifier.parse_next(&mut input.input)
}

fn code<'s, 'b>(input: &mut Input<'s, 'b>) -> PResult<Code<'s>> {
    "~~~".parse_next(input)?;
    let lang = opt(preceded(space0, map_identifier)).parse_next(input)?;
    preceded(space0, line_ending).parse_next(input)?;
    let (value, _) = advance_to(fence("~~~"), '~').parse_next(input)?;

    Ok(Code {
        value,
        lang,
        meta: None,
    })
}

fn strikethrough<'s, 'b>(input: &mut Input<'s, 'b>) -> PResult<NodeVec> {
    "~~".parse_next(input)?;
    let children = paragraph('~').parse_next(input)?;
    "~~".parse_next(input)?;

    Ok(children)
}

fn image<'s, 'b>(input: &mut Input<'s, 'b>) -> PResult<Image<'s>> {
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

fn heading<'s, 'b>(input: &mut Input<'s, 'b>) -> PResult<Heading> {
    let depth = take_while(1..256, '#').parse_next(input)?.len() as u8;
    let children = paragraph(('\r', '\n')).parse_next(input)?;
    line_ending(input)?;

    Ok(Heading { children, depth })
}

fn map_element<'s, 'b>(input: &mut Input<'s, 'b>) -> PResult<Element<'s>> {
    wincomp::parse::element.parse_next(&mut input.input)
}

fn top<'s, 'b>(input: &mut Input<'s, 'b>) -> PResult<NodeId> {
    let result = terminated(
        winnow::combinator::dispatch! {peek(any);
            '-' => yaml.map(Node::Yaml),
            '<' => map_element.map(Node::Html),
            '~' => code.map(Node::Code),
            '$' => math.map(Node::Math),
            '#' => heading.map(Node::Heading),
            _ => fail::<_, Node, _>,
        },
        multispace0,
    )
    .parse_next(input);

    let node = match result {
        Ok(n) => n,
        Err(ErrMode::Backtrack(_)) => terminated(paragraph(('\r', '\n')), multispace0)
            .map(Node::Paragraph)
            .parse_next(input)?,
        Err(e) => return Err(e),
    };

    Ok(input.state.insert(node))
}

fn link<'s, 'b>(input: &mut Input<'s, 'b>) -> PResult<Link<'s>> {
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

fn strong<'s, 'b>(input: &mut Input<'s, 'b>) -> PResult<NodeVec> {
    // TODO: not quite right since this may trip on something like
    // **strong * stuff**
    delimited("**", paragraph('*'), "**").parse_next(input)
}

fn inline_node<'s, 'b>(input: &mut Input<'s, 'b>) -> PResult<Node<'s>> {
    winnow::combinator::dispatch! {peek(any);
        '*' => strong.map(Node::Strong).context(StrContext::Label("strong")),
        '[' => link.map(Node::Link).context(StrContext::Label("link")),
        '!' => image.map(Node::Image).context(StrContext::Label("image")),
        '~' => strikethrough.map(Node::Delete).context(StrContext::Label("delete")),
        '$' => inline_math.map(Node::InlineMath).context(StrContext::Label("inline math")),
        '`' => inline_code.map(Node::InlineCode).context(StrContext::Label("inline code")),
        _ => fail::<_, Node, _>,
    }
    .parse_next(input)
}

fn paragraph<C>(termination: C) -> impl for<'s, 'b> FnMut(&mut Input<'s, 'b>) -> PResult<NodeVec>
where
    C: ContainsToken<char>,
{
    move |input| {
        let checkpoint = input.checkpoint();
        let mut string = input.input;
        let mut nodes = tinyvec::TinyVec::new();

        let mut iter = string.char_indices();
        loop {
            let Some((i, c)) = iter.next() else {
                if string.len() > 0 {
                    nodes.push(input.state.insert(Node::Text(string)));
                }
                break;
            };

            if termination.contains_token(c) {
                if i != 0 {
                    nodes.push(input.state.insert(Node::Text(&string[..i])));
                    input.input = &string[i..];
                }
                break;
            }

            match c {
                '*' | '[' | '!' | '~' | '$' | '`' => {
                    input.input = &string[i..];
                    match inline_node.parse_next(input) {
                        Ok(node) => {
                            if i != 0 {
                                nodes.push(input.state.insert(Node::Text(&string[..i])));
                            }
                            nodes.push(input.state.insert(node));
                            string = input.input;
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
            Ok(NodeVec(nodes))
        }
    }
}

fn document<'s, 'b>(input: &mut Input<'s, 'b>) -> PResult<NodeVec> {
    preceded(multispace0, repeat(0.., top)).parse_next(input)
}

#[derive(Debug)]
pub struct Document {
    pub nodes: NodeVec,
}

impl Document {
    pub fn parse<'s, 'b>(
        input: &'s str,
        arena: &'b mut NodeArena<'s>,
    ) -> Result<Self, ParseError<Input<'s, 'b>, ContextError>> {
        let input = Input {
            input,
            state: arena,
        };
        let nodes = document.parse(input)?;

        Ok(Self { nodes })
    }
}

// #[cfg(test)]
// mod test {
//     use super::*;
//
//     #[test]
//     fn test_node() {
//         let result = inline_node.parse(&mut "`code`").unwrap();
//
//         assert!(matches!(result, Node::InlineCode(c) if c == "code"));
//     }
//
//     #[test]
//     fn test_doc() {
//         let mut input = "
// # Hello, world!
//
// How are `you` doing?
//
//
// [Here's a link!](wikipedia.com)
//
// <Text>
//     Here's some html!
// </Text>
// ";
//
//         let result = document.parse(&mut input);
//
//         panic!("{result:#?}");
//
//         match result {
//             Ok(r) => assert_eq!(r.len(), 3),
//             Err(e) => {
//                 panic!("{e}");
//             }
//         }
//     }
// }
