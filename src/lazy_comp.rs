use std::{
    collections::HashMap,
    hash::BuildHasher,
    ops::Deref,
    sync::{RwLock, RwLockReadGuard},
};
use wincomp::Component;

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

pub struct LazyComponent<'s> {
    raw: &'s str,
    component: RwLock<Option<Component<'s>>>,
}

pub struct ComponentGuard<'a, 's>(RwLockReadGuard<'a, Option<Component<'s>>>);

impl<'a, 's> Deref for ComponentGuard<'a, 's> {
    type Target = Component<'s>;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().unwrap()
    }
}

impl<'s> LazyComponent<'s> {
    pub const fn new(raw: &'s str) -> Self {
        Self {
            raw,
            component: RwLock::new(None),
        }
    }

    pub fn component(&self) -> ComponentGuard<'_, 's> {
        let component = self.component.read().unwrap();

        if component.is_some() {
            ComponentGuard(component)
        } else {
            drop(component);

            let mut component = self.component.write().unwrap();

            /// It's possible this was set to Some before locking.
            if component.is_none() {
                let data = Component::new(self.raw).expect("Lazy components should be well-formed");
                *component = Some(data);
            }
            drop(component);

            let component = self.component.read().unwrap();
            ComponentGuard(component)
        }
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
    pub fn get(&self, name: &str) -> Option<ComponentGuard<'_, 's>> {
        self.0.get(name).map(|e| e.component())
    }
}
