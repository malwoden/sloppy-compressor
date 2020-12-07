#[derive(PartialEq, Debug)]
pub struct Node {
    pub offset: u16,
    pub length: u16,
    pub char: u8,
}

#[derive(PartialEq, Debug)]
pub enum NodeType {
    ByteLiteral { lit: u8 },
    Reference { offset: u16, length: u16 },
    EndOfStream,
}
