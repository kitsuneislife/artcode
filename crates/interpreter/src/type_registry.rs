use std::collections::HashMap;
use core::Token;

#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<(String, String)>,
    pub methods: std::collections::HashMap<String, core::ast::Function>,
}

#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<(String, Option<Vec<String>>)>,
    pub methods: std::collections::HashMap<String, core::ast::Function>,
}

#[derive(Debug, Clone)]
pub struct TypeRegistry {
    pub structs: HashMap<String, StructDef>,
    pub enums: HashMap<String, EnumDef>,
}

impl TypeRegistry {
    pub fn new() -> Self {
        TypeRegistry {
            structs: HashMap::new(),
            enums: HashMap::new(),
        }
    }

    pub fn register_struct(&mut self, name: Token, fields: Vec<(Token, String)>) {
        let struct_def = StructDef {
            name: name.lexeme.clone(),
            fields: fields.into_iter().map(|(token, ty)| (token.lexeme, ty)).collect(),
            methods: std::collections::HashMap::new(),
        };
        self.structs.insert(name.lexeme, struct_def);
    }

    pub fn register_enum(&mut self, name: Token, variants: Vec<(Token, Option<Vec<String>>)>) {
        let enum_def = EnumDef {
            name: name.lexeme.clone(),
            variants: variants.into_iter().map(|(token, params)| (token.lexeme, params)).collect(),
            methods: std::collections::HashMap::new(),
        };
        self.enums.insert(name.lexeme, enum_def);
    }

    pub fn get_struct(&self, name: &str) -> Option<&StructDef> {
        self.structs.get(name)
    }

    pub fn get_enum(&self, name: &str) -> Option<&EnumDef> {
        self.enums.get(name)
    }

    pub fn has_enum(&self, name: &str) -> bool {
        self.enums.contains_key(name)
    }

    pub fn has_struct(&self, name: &str) -> bool {
        self.structs.contains_key(name)
    }
}

impl Default for TypeRegistry {
    fn default() -> Self { Self::new() }
}