//! Testing framework for Sentio SDK processors
//!
//! This module provides a comprehensive testing framework that allows both SDK developers
//! and users to test their processors with simulated blockchain data. The framework mirrors
//! the TypeScript testing capabilities while leveraging Rust's type system.
//!
//! # Architecture
//!
//! The testing framework consists of several key components:
//!
//! - [`TestProcessorServer`]: The main orchestrator that manages processor lifecycle and coordinates tests
//! - [`TestProvider`]: Handles test environment configuration and endpoint setup
//! - [`MemoryDatabase`]: In-memory storage for testing metrics and events without external dependencies
//! - Chain-specific facets (e.g., [`EthTestFacet`]): Simulate blockchain-specific data and events
//!
//! # Usage
//!
//! ```rust
//! use sentio_sdk::testing::TestProcessorServer;
//!
//! #[tokio::test]
//! async fn test_my_processor() {
//!     let server = TestProcessorServer::new(|| {
//!         // Initialize your processor here
//!     }).await;
//!     
//!     // Test Ethereum log processing
//!     let log_result = server.eth.test_log(mock_transfer_log()).await;
//!     assert_eq!(log_result.counters.len(), 1);
//! }
//! ```

pub mod test_processor_server;
pub mod test_provider;
pub mod memory_database;
pub mod facets;
pub mod utils;

pub use test_processor_server::*;
pub use test_provider::*;
pub use memory_database::*;
pub use facets::*;
pub use utils::*;