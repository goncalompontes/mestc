use std::collections::{BTreeMap, HashMap};

use crate::{
    ast::Ident,
    hir::{Type, TypeVar},
};

pub struct Scheme;

pub type TyVar = TypeVar;
pub type TmVar = Ident;
pub type Env = BTreeMap<TmVar, Scheme>;

pub struct Subst {
    pub types: HashMap<TyVar, Type>,
}

pub struct TypeInference {
    counter: u64,
}
