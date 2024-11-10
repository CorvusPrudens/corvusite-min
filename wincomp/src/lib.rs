use crate::element::{Element, Node};
use winnow::{
    ascii::multispace0,
    combinator::{delimited, terminated},
    error::{ContextError, ParseError},
    Parser,
};

pub mod element;
pub mod parse;

pub struct Document<'s> {
    pub nodes: Vec<Node<'s>>,
}

pub struct Component<'s> {
    pub root: Element<'s>,
}

impl<'s> Component<'s> {
    pub fn new(mut source: &'s str) -> Result<Self, ParseError<&'s str, ContextError>> {
        let root = delimited(multispace0, parse::element, multispace0).parse(&mut source)?;

        Ok(Self { root })
    }
}

impl<'s> Document<'s> {
    pub fn new(mut source: &'s str) -> Result<Self, ParseError<&'s str, ContextError>> {
        let nodes = terminated(parse::nodes, multispace0).parse(&mut source)?;

        Ok(Self { nodes })
    }

    pub fn expand<F>(&mut self, mut components: F)
    where
        F: FnMut(&str) -> Option<&Component<'s>>,
    {
        loop {
            if !Self::expand_recurse(&mut self.nodes, &mut components) {
                break;
            }
        }
    }

    fn expand_recurse<F>(nodes: &mut Vec<Node<'s>>, components: &mut F) -> bool
    where
        F: FnMut(&str) -> Option<&Component<'s>>,
    {
        let mut mutated = false;
        let mut index = 0;
        while index < nodes.len() {
            let Some(child) = nodes[index].element_mut() else {
                index += 1;
                continue;
            };

            if let Some(component) = components(child.name) {
                mutated = true;
                let declared_attributes = &component.root.attributes;
                let mut replacement_attributes = Vec::with_capacity(declared_attributes.len());

                for attribute in declared_attributes {
                    if let Some(attr) = child.attributes.iter().find(|a| a.name == attribute.name) {
                        replacement_attributes.push(*attr);
                    } else {
                        replacement_attributes.push(*attribute);
                    }
                }

                let mut component_copy = component.root.clone();

                // Assign properties
                element::walk(&mut component_copy, &mut |element| {
                    for attr in element.attributes.iter_mut() {
                        if let Some(value) = replacement_attributes.iter().find_map(|a| {
                            attr.value.is_some_and(|v| v == a.name).then_some(a.value)
                        }) {
                            attr.value = value;
                        }
                    }
                });

                let mut children = std::mem::take(&mut child.children);

                let mut inner_index = 0;
                let outlet = element::find_mut(&mut component_copy, &mut |el| {
                    if let Some(i) = el.children.iter().enumerate().find_map(|(i, c)| {
                        if c.element().is_some_and(|e| e.name == "children") {
                            Some(i)
                        } else {
                            None
                        }
                    }) {
                        inner_index = i;
                        true
                    } else {
                        false
                    }
                });

                if let Some(outlet) = outlet {
                    outlet.children.remove(inner_index);
                    outlet.children.append(&mut children)
                }

                nodes.remove(index);

                for (i, child) in component_copy.children.into_iter().enumerate() {
                    nodes.insert(index + i, child);
                }
            }

            // TODO: technically this can panic if the component has no children
            let Some(child) = nodes[index].element_mut() else {
                index += 1;
                continue;
            };

            mutated |= Self::expand_recurse(&mut child.children, components);

            index += 1;
        }

        mutated
    }
}

impl Element<'_> {
    pub fn write<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        write!(writer, "<{}", self.name)?;

        for attribute in self.attributes.iter() {
            write!(writer, " {}", attribute.name)?;

            if let Some(value) = attribute.value {
                write!(writer, r#"="{value}""#)?;
            }
        }

        if self.children.is_empty() {
            write!(writer, "/>")?;
        } else {
            write!(writer, ">")?;

            Document::write_element(writer, &self.children)?;

            write!(writer, "</{}>", self.name)?;
        }

        Ok(())
    }
}

impl Document<'_> {
    pub fn write<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        write!(writer, "<!DOCTYPE html>")?;
        Self::write_element(writer, &self.nodes)
    }

    fn write_element<W: std::io::Write>(writer: &mut W, nodes: &[Node<'_>]) -> std::io::Result<()> {
        for node in nodes {
            match node {
                Node::Element(element) => element.write(writer)?,
                Node::Text(t) => {
                    writer.write(t.as_bytes())?;
                }
                Node::Comment(_) => {}
            }
        }

        Ok(())
    }
}
