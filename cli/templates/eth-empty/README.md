# {{PROJECT_NAME}}

A Sentio processor for Ethereum blockchain data processing.

## Getting Started

1. **Configure your processor**: Update the contract address and chain ID in `src/processor.rs`
2. **Define your schema**: Add entity definitions to `schema.graphql`
3. **Implement event handlers**: Add your event processing logic in `src/processor.rs`
4. **Build**: Run `cargo build` or use `sentio build`
5. **Test**: Run `cargo test` or use `sentio test`

## Project Structure

- `src/main.rs` - Entry point and server setup
- `src/processor.rs` - Main processor logic and event handlers
- `src/lib.rs` - Library exports
- `schema.graphql` - Entity schema definitions
- `build.rs` - Build script for code generation
- `Cargo.toml` - Project configuration and dependencies

## Development

To add a new event handler:

1. Define an event marker struct
2. Implement `EventMarker` trait with event filters
3. Implement `EthEventHandler` trait with processing logic
4. Configure the event in `main.rs`

## Commands

- `sentio build` - Build the processor
- `sentio test` - Run tests
- `sentio upload` - Upload to Sentio platform