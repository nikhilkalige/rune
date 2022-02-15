use std::ops::Deref;
use std::{ops::Index, slice::SliceIndex};

use crate::data::Environment;
use crate::hashmap::HashMap;
use crate::object::Object;
use crate::symbol::Symbol;

use super::Arena;

#[repr(transparent)]
#[derive(Default, Debug, PartialEq)]
pub(crate) struct GcStore<'ob> {
    obj: Object<'ob>,
}

impl<'ob> GcStore<'ob> {
    fn set(&mut self, obj: Object<'ob>) {
        self.obj = obj;
    }
}

impl<'ob> From<Object<'ob>> for GcStore<'ob> {
    fn from(obj: Object<'ob>) -> Self {
        Self { obj }
    }
}

#[repr(transparent)]
pub(crate) struct Gc<T: ?Sized> {
    data: T,
}

impl<T> Deref for Gc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> AsRef<T> for Gc<T> {
    fn as_ref(&self) -> &T {
        &self.data
    }
}

impl<T> Gc<T> {
    pub(crate) unsafe fn new(data: T) -> Self {
        Gc { data }
    }

    pub(crate) fn mutate(&mut self, func: fn(&mut T)) {
        let inner = &mut self.data;
        func(inner);
    }

    pub(crate) fn add<'ob, 'a>(&mut self, obj: Object<'ob>, func: fn(&'a mut T, GcStore<'a>)) {
        let inner = unsafe { std::mem::transmute::<&mut T, &'a mut T>(&mut self.data) };
        let store = unsafe { std::mem::transmute::<Object<'ob>, GcStore<'a>>(obj) };
        func(inner, store);
    }

    pub(crate) fn insert<'ob, 'a, K: 'static>(
        &mut self,
        key: K,
        obj: Object<'ob>,
        func: fn(&'a mut T, K, GcStore<'a>),
    ) {
        let inner = unsafe { std::mem::transmute::<&mut T, &'a mut T>(&mut self.data) };
        let store = unsafe { std::mem::transmute::<Object<'ob>, GcStore<'a>>(obj) };
        func(inner, key, store);
    }
}

impl<'root> Gc<GcStore<'root>> {
    pub(crate) fn obj(&self) -> Object {
        unsafe { Object::from_raw(self.data.obj.into()) }
    }

    pub(crate) fn bind<'ob>(&self, _cx: &'ob Arena) -> Object<'ob> {
        unsafe { Object::from_raw(self.data.obj.into()) }
    }
}

impl<'ob, 'root> AsRef<Object<'ob>> for Gc<GcStore<'root>> {
    fn as_ref(&self) -> &Object<'ob> {
        unsafe { &*(self as *const Self).cast::<Object>() }
    }
}

impl<'ob, 'root> AsRef<[Object<'ob>]> for Gc<[GcStore<'root>]> {
    fn as_ref(&self) -> &[Object<'ob>] {
        let ptr = self.data.as_ptr().cast::<Object>();
        let len = self.data.len();
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}

impl<T, U> Gc<(T, U)> {
    pub(crate) fn get(&self) -> &(Gc<T>, Gc<U>) {
        // SAFETY: `Gc<T>` has the same memory layout as `T`.
        unsafe { &*(self as *const Gc<(T, U)>).cast::<(Gc<T>, Gc<U>)>() }
    }
}

impl<T, I: SliceIndex<[T]>> Index<I> for Gc<Vec<T>> {
    type Output = Gc<I::Output>;

    fn index(&self, index: I) -> &Self::Output {
        unsafe { &*(Index::index(&self.data, index) as *const I::Output as *const Gc<I::Output>) }
    }
}

impl<T, I: SliceIndex<[T]>> Index<I> for Gc<[T]> {
    type Output = Gc<I::Output>;

    fn index(&self, index: I) -> &Self::Output {
        unsafe { &*(Index::index(&self.data, index) as *const I::Output as *const Gc<I::Output>) }
    }
}

impl<T> Gc<Vec<T>> {
    pub(crate) fn as_slice_of_gc(&self) -> &[Gc<T>] {
        // SAFETY: `Gc<T>` has the same memory layout as `T`.
        unsafe { &*(self.data.as_slice() as *const [T] as *const [Gc<T>]) }
    }
}

impl<T> Gc<[T]> {
    pub(crate) fn as_slice_of_gc(&self) -> &[Gc<T>] {
        // SAFETY: `Gc<T>` has the same memory layout as `T`.
        unsafe { &*(self as *const Gc<[T]> as *const [Gc<T>]) }
    }
}

impl<K, V> Gc<HashMap<K, V>>
where
    K: Eq + std::hash::Hash,
{
    pub(crate) fn get_obj<Q: ?Sized>(&self, k: &Q) -> Option<&Gc<V>>
    where
        K: std::borrow::Borrow<Q>,
        Q: std::hash::Hash + Eq,
    {
        self.data
            .get(k)
            .map(|v| unsafe { &*(v as *const V).cast::<Gc<V>>() })
    }

    pub(crate) fn get_gc<Q: ?Sized>(&mut self, k: &Q) -> Option<&mut Gc<V>>
    where
        K: std::borrow::Borrow<Q>,
        Q: std::hash::Hash + Eq,
    {
        self.data
            .get_mut(k)
            .map(|v| unsafe { &mut *(v as *mut V).cast::<Gc<V>>() })
    }
}

type Prop<'rt> = Gc<HashMap<Symbol, Vec<(Symbol, GcStore<'rt>)>>>;
impl<'rt> Gc<Environment<'rt>> {
    pub(crate) fn vars(&self) -> &Gc<HashMap<Symbol, GcStore<'rt>>> {
        unsafe { &*(&self.data.vars as *const HashMap<_, _>).cast() }
    }

    pub(crate) fn props(&self) -> &Prop<'rt> {
        unsafe { &*(&self.data.props as *const HashMap<_, _>).cast() }
    }
}

#[cfg(test)]
mod test {
    use crate::arena::{Arena, RootSet};

    use super::*;

    #[test]
    fn indexing() {
        let root = &RootSet::default();
        let arena = &Arena::new(root);
        let mut vec: Gc<Vec<GcStore>> = Gc { data: vec![] };

        vec.add(Object::NIL, Vec::push);
        assert!(matches!(vec[0].obj(), Object::Nil(_)));
        let str1 = arena.add("str1");
        let str2 = arena.add("str2");
        vec.add(str1, Vec::push);
        vec.add(str2, Vec::push);
        let slice = &vec[0..3];
        assert_eq!(vec![Object::NIL, str1, str2], slice.as_ref());
    }
}
