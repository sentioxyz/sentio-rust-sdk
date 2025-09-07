//! Code generation module for entities

pub mod generator;
pub mod entity_gen;

pub use generator::{EntityGenerator, GenerationOptions};
pub use entity_gen::EntityCodeGenerator;
