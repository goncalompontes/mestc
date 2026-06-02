use crate::ast::{Env, EvalError, ExprKind, Value};
use lasso::Rodeo;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct Thunk<'bump> {
    expr: Option<&'bump ExprKind<'bump>>,
    env: Rc<RefCell<Env<'bump>>>,
    value: Rc<RefCell<Option<Value<'bump>>>>,
}

impl<'bump> Thunk<'bump> {
    pub fn new(expr: &'bump ExprKind<'bump>, env: Env<'bump>) -> Self {
        Self {
            expr: Some(expr),
            env: Rc::new(RefCell::new(env)),
            value: Rc::new(RefCell::new(None)),
        }
    }

    pub fn new_shared(expr: &'bump ExprKind<'bump>, env: Rc<RefCell<Env<'bump>>>) -> Self {
        Self {
            expr: Some(expr),
            env,
            value: Rc::new(RefCell::new(None)),
        }
    }

    pub fn from_value(value: Value<'bump>, env: Env<'bump>) -> Self {
        Self {
            expr: None,
            env: Rc::new(RefCell::new(env)),
            value: Rc::new(RefCell::new(Some(value))),
        }
    }

    pub fn env_cell(&self) -> Rc<RefCell<Env<'bump>>> {
        Rc::clone(&self.env)
    }

    pub fn force(&self, rodeo: &Rodeo) -> Result<Value<'bump>, EvalError> {
        if let Some(val) = self.value.borrow().as_ref() {
            return Ok(val.clone());
        }
        let env = self.env.borrow().clone();
        let val = self.expr.unwrap().eval_lazy(&env, rodeo)?;
        *self.value.borrow_mut() = Some(val.clone());
        Ok(val)
    }
}
