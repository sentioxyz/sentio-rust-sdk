/// Types of handlers supported by Ethereum processors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EthHandlerType {
    /// Event handler for contract events
    Event,
    /// Call handler for contract calls
    Call,
    /// Block handler for processing blocks
    Block,
    /// Transaction handler for processing transactions
    Transaction,
}

impl std::fmt::Display for EthHandlerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EthHandlerType::Event => write!(f, "event"),
            EthHandlerType::Call => write!(f, "call"),
            EthHandlerType::Block => write!(f, "block"),
            EthHandlerType::Transaction => write!(f, "transaction"),
        }
    }
}