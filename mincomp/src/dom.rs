use html5ever::tendril::StrTendril;
use html5ever::{interface::NodeOrText, QualName};
use slotmap::{HopSlotMap, SlotMap};
use std::borrow::Cow;
use std::cell::RefCell;

mod expand;
mod output;
mod tree;

slotmap::new_key_type! {
    pub struct NameId;
}

slotmap::new_key_type! {
    pub struct NodeId;
}

#[derive(Debug)]
pub struct Node {
    name: NameId,
    parent: Option<NodeId>,
    children: Vec<Child>,
    attributes: Vec<html5ever::Attribute>,
}

#[derive(Debug)]
pub enum Child {
    Node(NodeId),
    Text(StrTendril),
}

impl Child {
    pub fn node(&self) -> Option<NodeId> {
        match self {
            Self::Node(n) => Some(*n),
            _ => None,
        }
    }
}

impl From<NodeOrText<NodeId>> for Child {
    fn from(value: NodeOrText<NodeId>) -> Self {
        match value {
            NodeOrText::AppendNode(node) => Self::Node(node),
            NodeOrText::AppendText(text) => Self::Text(text),
        }
    }
}

#[derive(Debug)]
pub struct SharedDom {
    document: NodeId,
    nodes: RefCell<SlotMap<NodeId, Node>>,
    names: RefCell<HopSlotMap<NameId, QualName>>,
    errors: RefCell<Vec<Cow<'static, str>>>,
}

#[derive(Debug)]
pub struct Dom {
    root: NodeId,
    nodes: SlotMap<NodeId, Node>,
    names: HopSlotMap<NameId, QualName>,
    errors: Vec<Cow<'static, str>>,
}

impl SharedDom {
    pub fn new() -> Self {
        let mut nodes = SlotMap::<NodeId, _>::default();
        let mut names = HopSlotMap::<NameId, _>::default();

        let name = names.insert(QualName::new(None, "".into(), "document".into()));

        let document = nodes.insert(Node {
            name,
            parent: None,
            children: vec![],
            attributes: vec![],
        });

        Self {
            nodes: RefCell::new(nodes),
            names: RefCell::new(names),
            document,
            errors: RefCell::new(vec![]),
        }
    }
}
