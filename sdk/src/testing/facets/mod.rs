//! Chain-specific testing facets
//!
//! Each facet provides testing utilities for a specific blockchain ecosystem,
//! allowing simulation of blockchain events, transactions, and blocks for testing processors.

pub mod eth_facet;
// TODO: Add other chain facets as needed:
// pub mod aptos_facet;
// pub mod sui_facet; 
// pub mod solana_facet;

pub use eth_facet::*;