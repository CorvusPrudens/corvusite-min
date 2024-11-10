use std::{collections::HashMap, hash::BuildHasher, sync::OnceLock};
use wincomp::Component;

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

pub struct LazyComponent<'s> {
    raw: &'s str,
    component: OnceLock<Component<'s>>,
}

impl<'s> LazyComponent<'s> {
    pub const fn new(raw: &'s str) -> Self {
        Self {
            raw,
            component: OnceLock::new(),
        }
    }

    pub fn component(&self) -> &Component<'s> {
        self.component.get_or_init(|| {
            Component::new(self.raw).expect("Lazy components should be well-formed")
        })
    }
}

pub struct LazyComponents<'s, S>(HashMap<&'s str, LazyComponent<'s>, S>);

impl<'s, S, const LEN: usize> From<[(&'s str, LazyComponent<'s>); LEN]> for LazyComponents<'s, S>
where
    S: BuildHasher + Default,
{
    fn from(value: [(&'s str, LazyComponent<'s>); LEN]) -> Self {
        Self(HashMap::from_iter(value))
    }
}

impl<'s, S> LazyComponents<'s, S>
where
    S: BuildHasher,
{
    pub fn get(&self, name: &str) -> Option<&Component<'s>> {
        self.0.get(name).map(|e| e.component())
    }
}
