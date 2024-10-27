#[derive(Debug, Clone)]
pub struct Element<'s> {
    pub name: &'s str,
    pub attributes: Vec<Attribute<'s>>,
    pub children: Vec<Node<'s>>,
}

/// Descent the tree depth-first.
pub fn find_mut<'a, 's, F>(
    element: &'a mut Element<'s>,
    predicate: &mut F,
) -> Option<&'a mut Element<'s>>
where
    F: FnMut(&Element<'s>) -> bool,
{
    if predicate(&element) {
        return Some(element);
    }

    for child in element.children.iter_mut().filter_map(|c| c.element_mut()) {
        let result = find_mut(child, predicate);
        if result.is_some() {
            return result;
        }
    }

    None
}

pub fn find_map<'s, F, O>(element: &mut Element<'s>, predicate: &mut F) -> Option<O>
where
    F: FnMut(&mut Element<'s>) -> Option<O>,
{
    if let Some(o) = predicate(element) {
        return Some(o);
    }

    for child in element.children.iter_mut().filter_map(|c| c.element_mut()) {
        let result = find_map(child, predicate);
        if result.is_some() {
            return result;
        }
    }

    None
}

pub fn walk<'s, F>(element: &mut Element<'s>, walker: &mut F)
where
    F: FnMut(&mut Element<'s>),
{
    walker(element);

    for child in element.children.iter_mut().filter_map(|c| c.element_mut()) {
        walk(child, walker)
    }
}

impl<'s> Element<'s> {}

#[derive(Debug, Clone)]
pub enum Node<'s> {
    Text(&'s str),
    Element(Element<'s>),
    Comment(&'s str),
}

impl<'s> Node<'s> {
    pub fn is_text(&self) -> bool {
        matches!(self, Self::Text(_))
    }

    pub fn element(&self) -> Option<&Element<'s>> {
        match self {
            Self::Element(e) => Some(e),
            _ => None,
        }
    }

    pub fn element_mut(&mut self) -> Option<&mut Element<'s>> {
        match self {
            Self::Element(e) => Some(e),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Attribute<'s> {
    pub name: &'s str,
    pub value: Option<&'s str>,
}
