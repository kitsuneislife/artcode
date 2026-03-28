use crate::type_registry::TypeRegistry;
use core::ast::{ArtValue, Function};
use core::environment::Environment;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub fn struct_field_or_method(
    struct_name: &str,
    fields: &HashMap<String, ArtValue>,
    field: &core::Token,
    type_registry: &TypeRegistry,
) -> Option<ArtValue> {
    if let Some(v) = fields.get(&field.lexeme) {
        return Some(v.clone());
    }
    if let Some(sdef) = type_registry.structs.get(struct_name)
        && let Some(m) = sdef.methods.get(&field.lexeme)
    {
        let mut new_params = m.params.clone();
        let drop_self = new_params
            .first()
            .map(|p| p.name.lexeme.as_str() == "self")
            .unwrap_or(false);
        if drop_self {
            new_params.remove(0);
        }
        let base_env = match m.closure.upgrade() {
            Some(e) => e,
            None => Rc::new(RefCell::new(Environment::new(None, 0, None))),
        };
        let bound_env = Rc::new(RefCell::new(Environment::new(Some(base_env.clone()), base_env.borrow().depth + 1, None)));
        bound_env.borrow_mut().define(
            "self",
            ArtValue::StructInstance {
                struct_name: struct_name.to_string(),
                fields: fields.clone(),
            },
        );
        let bound_fn = Function {
            name: m.name.clone(),
            type_params: m.type_params.clone(),
            params: new_params,
            body: m.body.clone(),
            closure: Rc::downgrade(&bound_env),
            retained_env: Some(bound_env),
        };
        return Some(ArtValue::Function(Rc::new(bound_fn)));
    }
    None
}

pub fn enum_method(
    enum_name: &str,
    variant: &str,
    values: &[ArtValue],
    field: &core::Token,
    type_registry: &TypeRegistry,
) -> Option<ArtValue> {
    // Built-in methods for Option and Result
    if enum_name == "Result" || enum_name == "Option" {
        let enum_val = ArtValue::EnumInstance {
            enum_name: enum_name.to_string(),
            variant: variant.to_string(),
            values: values.to_vec(),
        };
        match field.lexeme.as_str() {
            "is_ok" | "is_some" => {
                return Some(ArtValue::Builtin(core::ast::BuiltinFn::EnumIsOk(Box::new(
                    enum_val,
                ))));
            }
            "is_err" | "is_none" => {
                return Some(ArtValue::Builtin(core::ast::BuiltinFn::EnumIsErr(
                    Box::new(enum_val),
                )));
            }
            "unwrap" => {
                return Some(ArtValue::Builtin(core::ast::BuiltinFn::EnumUnwrap(
                    Box::new(enum_val),
                )));
            }
            "unwrap_or" => {
                return Some(ArtValue::Builtin(core::ast::BuiltinFn::EnumUnwrapOr(
                    Box::new(enum_val),
                )));
            }
            _ => {}
        }
    }

    if let Some(edef) = type_registry.enums.get(enum_name)
        && let Some(m) = edef.methods.get(&field.lexeme)
    {
        let mut new_params = m.params.clone();
        let drop_self = new_params
            .first()
            .map(|p| p.name.lexeme.as_str() == "self")
            .unwrap_or(false);
        if drop_self {
            new_params.remove(0);
        }
        let base_env = match m.closure.upgrade() {
            Some(e) => e,
            None => Rc::new(RefCell::new(Environment::new(None, 0, None))),
        };
        let (base_depth, base_arena) = {
            let b = base_env.borrow();
            (b.depth, b.associated_arena)
        };
        let bound_env = Rc::new(RefCell::new(Environment::new(Some(base_env), base_depth + 1, base_arena)));
        bound_env.borrow_mut().define(
            "self",
            ArtValue::EnumInstance {
                enum_name: enum_name.to_string(),
                variant: variant.to_string(),
                values: values.to_vec(),
            },
        );
        let bound_fn = Function {
            name: m.name.clone(),
            type_params: m.type_params.clone(),
            params: new_params,
            body: m.body.clone(),
            closure: Rc::downgrade(&bound_env),
            retained_env: Some(bound_env),
        };
        return Some(ArtValue::Function(Rc::new(bound_fn)));
    }
    None
}
