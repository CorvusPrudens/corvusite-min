use crate::element::{Attribute, Element, Node};
use winnow::{
    ascii::multispace0,
    combinator::{alt, cut_err, delimited, dispatch, opt, peek, preceded, repeat},
    error::{AddContext, ContextError, ErrMode, StrContext, StrContextValue},
    stream::Stream,
    token::{any, take_until, take_while},
    PResult, Parser,
};

pub fn identifier<'s>(input: &mut &'s str) -> PResult<&'s str> {
    any.verify(|c: &char| c.is_alphabetic())
        .parse_peek(*input)?;

    take_while(1.., |c: char| c.is_alphanumeric() || c == '_' || c == '-').parse_next(input)
}

fn parse_string<'s>(input: &mut &'s str) -> PResult<&'s str> {
    let checkpoint = input.checkpoint();
    '"'.parse_next(input)?;

    // go until we find another double quote not preceeded by a backslash
    let mut last_char = '"';
    for (i, char) in input.char_indices() {
        if char == '"' && last_char != '\\' {
            let string = &input[..i];
            *input = &input[i + 1..];
            return Ok(string);
        }

        last_char = char;
    }

    Err(ErrMode::Cut(ContextError::default().add_context(
        input,
        &checkpoint,
        StrContext::Expected(StrContextValue::CharLiteral('"')),
    )))
}

fn attribute<'s>(input: &mut &'s str) -> PResult<Attribute<'s>> {
    let name = identifier.parse_next(input)?;
    let value = opt((delimited(multispace0, '=', multispace0), parse_string))
        .parse_next(input)?
        .map(|(_, string)| string);

    Ok(Attribute { name, value })
}

fn node<'s>(input: &mut &'s str) -> PResult<Node<'s>> {
    let mut bracket_parser = preceded(
        multispace0,
        alt((
            preceded(
                "<!--",
                advance_to::<_, _, ContextError>("-->", '-').map(|(text, _)| Node::Comment(text)),
            ),
            preceded(peek("<"), element.map(Node::Element)),
        )),
    );

    dispatch! {peek(any);
        '<' => bracket_parser,
        _ => take_until(1.., '<').map(Node::Text)
    }
    .context(StrContext::Label("tag or text"))
    .parse_next(input)
}

pub(crate) fn nodes<'s>(input: &mut &'s str) -> PResult<Vec<Node<'s>>> {
    repeat(0.., node).parse_next(input)
}

fn closing_tag<'a>(name: &'a str) -> impl Fn(&mut &str) -> PResult<()> + 'a {
    move |input| {
        ("</", delimited(multispace0, name, multispace0), ">")
            .map(|_| ())
            .parse_next(input)
    }
}

pub fn advance_to<'s, P, O, E>(
    mut parser: P,
    hint: char,
) -> impl FnMut(&mut &'s str) -> PResult<(&'s str, O)>
where
    P: Parser<&'s str, O, E>,
{
    move |input| {
        let start = *input;
        let checkpoint = input.checkpoint();

        for (i, c) in input.char_indices() {
            let new_input = &mut &input[i..];

            if c == hint {
                if let Ok(p) = parser.parse_next(new_input) {
                    *input = new_input;
                    return Ok((&start[..i], p));
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

pub fn element<'s>(input: &mut &'s str) -> PResult<Element<'s>> {
    '<'.parse_next(input)?;

    let name = identifier.parse_next(input)?;
    let attributes = repeat(0.., preceded(multispace0, attribute)).parse_next(input)?;
    let close = preceded(multispace0, alt(("/>", ">"))).parse_next(input)?;

    match close {
        "/>" => Ok(Element {
            name,
            attributes,
            children: Vec::new(),
        }),
        ">" => match name {
            "script" | "style" => {
                let (text, _) = advance_to(closing_tag(name), '<').parse_next(input)?;

                Ok(Element {
                    name,
                    attributes,
                    children: vec![Node::Text(text)],
                })
            }
            "hr" | "input" | "link" | "img" => Ok(Element {
                name,
                attributes,
                children: vec![],
            }),
            _ => {
                let nodes = nodes.parse_next(input)?;
                cut_err(preceded(multispace0, closing_tag(name)))
                    .context(StrContext::Expected(StrContextValue::Description(
                        "closing tag",
                    )))
                    .parse_next(input)?;

                Ok(Element {
                    name,
                    attributes,
                    children: nodes,
                })
            }
        },
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_identifier() {
        assert_eq!(identifier.parse_next(&mut "test ").unwrap(), "test");
        assert_eq!(
            identifier.parse_next(&mut "test-kebab").unwrap(),
            "test-kebab"
        );
        assert_eq!(identifier.parse_next(&mut "alpha1").unwrap(), "alpha1");
    }

    #[test]
    fn test_element() {
        assert_eq!(element.parse_next(&mut "<test />").unwrap().name, "test");
        assert_eq!(
            element.parse_next(&mut "<test></test>").unwrap().name,
            "test"
        );

        let element = element
            .parse_next(&mut "<test><div /><div /></test>")
            .unwrap();

        assert_eq!(element.children.len(), 2);
    }

    #[test]
    fn test_text() {
        let children = element
            .parse_next(&mut "<test><div /></test>")
            .unwrap()
            .children;

        assert!(children.first().is_some_and(|c| !c.is_text()));

        let children = element
            .parse_next(&mut "<test>Here's some text!</test>")
            .unwrap()
            .children;

        assert!(children.first().is_some_and(|c| c.is_text()));
    }

    #[test]
    fn parses_string() {
        let input = &mut r#""test""#;

        assert_eq!(parse_string.parse_next(input).unwrap(), "test");
        assert_eq!(*input, "");
    }

    #[test]
    fn test_attr() {
        let attrs = element
            .parse_next(&mut r#"<test key="value" />"#)
            .unwrap()
            .attributes;
        assert_eq!(attrs[0].name, "key");
    }

    #[test]
    fn test_advance() {
        let js = r#"
            let test = 1 < 2;

            document.querySelectorAll(".prerender-link").forEach((link) => {
                link.addEventListener("mouseenter", () => {
                    addPrerenderRules(link.href);
                });
            });"#;

        let input = format!("{js}</script>");

        let (string, _) = advance_to(closing_tag("script"), '<')
            .parse_next(&mut input.as_str())
            .unwrap();

        assert_eq!(string, js);
    }

    #[test]
    fn test_small_component() {
        let mut component = "<Test>\n    <children />\n</Test>";

        let component = nodes.parse_next(&mut component);
        panic!("{component:#?}");
    }
}
