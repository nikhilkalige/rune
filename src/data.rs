use std::cell::RefCell;
use std::sync::Mutex;

use crate::arena::{GcRoot, Arena};
use crate::cons::Cons;
use crate::hashmap::{HashMap, HashSet};
use crate::object::{FuncCell, Object};
use crate::symbol::Symbol;
use crate::symbol::INTERNED_SYMBOLS;
use anyhow::{anyhow, Result};
use fn_macros::defun;
use lazy_static::lazy_static;

#[derive(Debug, Default, PartialEq)]
pub(crate) struct Environment {
    pub(crate) vars: HashMap<Symbol, GcRoot>,
    props: HashMap<Symbol, Vec<(Symbol, GcRoot)>>,
}

impl Environment {
    pub(crate) fn set_var(&mut self, sym: Symbol, value: Object) {
        unsafe {
            self.vars.insert(sym, GcRoot::new(value));
        }
    }
}

lazy_static! {
    pub(crate) static ref FEATURES: Mutex<HashSet<Symbol>> = Mutex::new({
        HashSet::with_capacity_and_hasher(0, std::hash::BuildHasherDefault::default())
    });
}

fn set_global_function(symbol: Symbol, func: FuncCell) {
    let map = INTERNED_SYMBOLS.lock().unwrap();
    map.set_func(symbol, func);
}

#[defun]
pub(crate) fn fset(symbol: Symbol, definition: Object) -> Result<Symbol> {
    if definition == Object::NIL {
        symbol.unbind_func();
    } else {
        let func = definition.try_into()?;
        set_global_function(symbol, func);
    }
    Ok(symbol)
}

#[defun]
pub(crate) fn defalias(
    symbol: Symbol,
    definition: Object,
    _docstring: Option<&String>,
) -> Result<Symbol> {
    fset(symbol, definition)
}

#[defun]
pub(crate) fn set<'ob>(
    place: Symbol,
    newlet: Object<'ob>,
    env: &mut Environment,
) -> Object<'ob> {
    env.set_var(place, newlet);
    newlet
}

#[defun]
pub(crate) fn put<'ob>(
    symbol: Symbol,
    propname: Symbol,
    value: Object<'ob>,
    env: &mut Environment,
) -> Object<'ob> {
    let rooted = unsafe {GcRoot::new(value)};
    match env.props.get_mut(&symbol) {
        Some(plist) => match plist.iter_mut().find(|x| x.0 == propname) {
            Some(x) => x.1 = rooted,
            None => plist.push((propname, rooted)),
        },
        None => {
            let plist = vec![(propname, rooted)];
            env.props.insert(symbol, plist);
        }
    }
    value
}

#[defun]
pub(crate) fn get<'ob>(symbol: Symbol, propname: Symbol, env: &Environment, arena: &'ob Arena) -> Object<'ob> {
    match env.props.get(&symbol) {
        Some(plist) => match plist.iter().find(|x| x.0 == propname) {
            Some((_, val)) => val.get(arena),
            None => Object::NIL,
        },
        None => Object::NIL,
    }
}

#[defun]
pub(crate) fn eq(obj1: Object, obj2: Object) -> bool {
    obj1.ptr_eq(obj2)
}

#[defun]
pub(crate) fn equal<'ob>(obj1: Object<'ob>, obj2: Object<'ob>) -> bool {
    obj1 == obj2
}

#[defun]
pub(crate) fn symbol_function<'ob>(symbol: Symbol) -> Object<'ob> {
    match symbol.func() {
        Some(f) => f.into(),
        None => Object::NIL,
    }
}

#[defun]
pub(crate) fn symbol_value<'ob>(symbol: Symbol, env: &mut Environment, arena: &'ob Arena) -> Option<Object<'ob>> {
    env.vars.get(&symbol).map(|x| x.get(arena))
}

#[defun]
pub(crate) fn symbol_name(symbol: Symbol) -> &'static str {
    symbol.name
}

#[defun]
pub(crate) fn null(obj: Object) -> bool {
    matches!(obj, Object::Nil(_))
}

#[defun]
pub(crate) fn fboundp(symbol: Symbol) -> bool {
    symbol.has_func()
}

#[defun]
pub(crate) fn fmakunbound(symbol: Symbol) -> Symbol {
    symbol.unbind_func();
    symbol
}

#[defun]
pub(crate) fn boundp(symbol: Symbol, env: &mut Environment) -> bool {
    env.vars.get(&symbol).is_some()
}

#[defun]
pub(crate) fn default_boundp(symbol: Symbol, env: &mut Environment) -> bool {
    env.vars.get(&symbol).is_some()
}

#[defun]
pub(crate) fn listp(object: Object) -> bool {
    matches!(object, Object::Nil(_) | Object::Cons(_))
}

#[defun]
pub(crate) fn nlistp(object: Object) -> bool {
    !listp(object)
}

#[defun]
pub(crate) fn symbolp(object: Object) -> bool {
    matches!(object, Object::Symbol(_))
}

#[defun]
pub(crate) fn functionp(object: Object) -> bool {
    matches!(object, Object::LispFn(_) | Object::SubrFn(_))
}

#[defun]
pub(crate) fn stringp(object: Object) -> bool {
    matches!(object, Object::String(_))
}

#[defun]
pub(crate) fn numberp(object: Object) -> bool {
    matches!(object, Object::Int(_) | Object::Float(_))
}

#[defun]
pub(crate) fn vectorp(object: Object) -> bool {
    matches!(object, Object::Vec(_))
}

#[defun]
pub(crate) fn consp(object: Object) -> bool {
    matches!(object, Object::Cons(_))
}

#[defun]
pub(crate) fn atom(object: Object) -> bool {
    !consp(object)
}

#[defun]
pub(crate) fn defvar<'ob>(
    symbol: Symbol,
    initvalue: Option<Object<'ob>>,
    _docstring: Option<&String>,
    env: &mut Environment,
) -> Object<'ob> {
    let value = initvalue.unwrap_or_default();
    set(symbol, value, env)
}

#[defun]
pub(crate) fn make_variable_buffer_local(variable: Symbol) -> Symbol {
    // TODO: Implement
    variable
}

#[defun]
pub(crate) fn aset<'ob>(
    array: &RefCell<Vec<Object<'ob>>>,
    idx: usize,
    newlet: Object<'ob>,
) -> Result<Object<'ob>> {
    let mut vec = array.try_borrow_mut()?;
    if idx < vec.len() {
        vec[idx] = newlet;
        Ok(newlet)
    } else {
        Err(anyhow!(
            "index {} is out of bounds. Length was {}",
            idx,
            vec.len()
        ))
    }
}

#[defun]
pub(crate) fn indirect_function(object: Object) -> Object {
    match object {
        Object::Symbol(sym) => match sym.resolve_callable() {
            Some(func) => func.into(),
            None => Object::NIL,
        },
        x => x,
    }
}

#[defun]
pub(crate) fn provide(feature: Symbol, _subfeatures: Option<&Cons>) -> Symbol {
    FEATURES.lock().unwrap().insert(feature);
    feature
}

defsubr!(
    eq,
    equal,
    set,
    put,
    get,
    defvar,
    make_variable_buffer_local,
    fset,
    aset,
    defalias,
    provide,
    symbol_function,
    symbol_value,
    symbol_name,
    null,
    fmakunbound,
    fboundp,
    boundp,
    default_boundp,
    listp,
    nlistp,
    stringp,
    symbolp,
    functionp,
    vectorp,
    numberp,
    consp,
    atom,
    indirect_function,
);
