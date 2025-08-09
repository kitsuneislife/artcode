use core::ast::{ArtValue, Function};
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use core::environment::Environment;
use crate::type_registry::TypeRegistry;

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
        && let Some(m) = sdef.methods.get(&field.lexeme) {
        let mut new_params = m.params.clone();
        let drop_self = new_params.first().map(|p| p.name.lexeme.as_str()=="self").unwrap_or(false);
        if drop_self { new_params.remove(0); }
        let bound_env = Rc::new(RefCell::new(Environment::new(Some(m.closure.clone()))));
        bound_env.borrow_mut().define("self", ArtValue::StructInstance { struct_name: struct_name.to_string(), fields: fields.clone() });
        let bound_fn = Function { name: m.name.clone(), params: new_params, body: m.body.clone(), closure: bound_env };
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
    if let Some(edef) = type_registry.enums.get(enum_name)
        && let Some(m) = edef.methods.get(&field.lexeme) {
        let mut new_params = m.params.clone();
        let drop_self = new_params.first().map(|p| p.name.lexeme.as_str()=="self").unwrap_or(false);
        if drop_self { new_params.remove(0); }
        let bound_env = Rc::new(RefCell::new(Environment::new(Some(m.closure.clone()))));
        bound_env.borrow_mut().define("self", ArtValue::EnumInstance { enum_name: enum_name.to_string(), variant: variant.to_string(), values: values.to_vec() });
        let bound_fn = Function { name: m.name.clone(), params: new_params, body: m.body.clone(), closure: bound_env };
        return Some(ArtValue::Function(Rc::new(bound_fn)));
    }
    None
}
