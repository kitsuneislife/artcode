use core::TokenType;
use std::collections::HashMap;

pub fn make_keywords() -> HashMap<String, TokenType> {
    let mut keywords = HashMap::new();
    keywords.insert("let".to_string(), TokenType::Let);
    keywords.insert("if".to_string(), TokenType::If);
    keywords.insert("else".to_string(), TokenType::Else);
    keywords.insert("true".to_string(), TokenType::True);
    keywords.insert("false".to_string(), TokenType::False);
    keywords.insert("and".to_string(), TokenType::And);
    keywords.insert("or".to_string(), TokenType::Or);
    keywords.insert("struct".to_string(), TokenType::Struct);
    keywords.insert("enum".to_string(), TokenType::Enum);
    keywords.insert("match".to_string(), TokenType::Match);
    keywords.insert("case".to_string(), TokenType::Case);
    keywords.insert("func".to_string(), TokenType::Func);
    keywords.insert("return".to_string(), TokenType::Return);
    keywords.insert("weak".to_string(), TokenType::Weak);
    keywords.insert("unowned".to_string(), TokenType::Unowned);
    keywords
}
