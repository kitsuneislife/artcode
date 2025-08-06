#[derive(Copy, Clone, Eq, PartialEq)]
pub enum Precedence {
    None,
    Assignment,
    Or,
    And,
    Equality,
    Comparison,
    Term,
    Factor,
    Unary,
    Call,
    Try,
}

impl Precedence {
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}