# Ethereum Basic Processor Example

This example demonstrates how to create a basic Ethereum processor using the Sentio SDK with the new synchronous ProcessorV3 API.

## Key Features

- **No Tokio dependency required**: The server manages its own async runtime
- **Command line argument support**: Built-in CLI for port and debug options
- **Structured logging**: Automatic log setup based on debug flag
- **ProcessorV3 implementation**: Uses the latest gRPC service definition

## Command Line Arguments

- `--port, -p <PORT>`: Port to listen on (default: 50051)
- `--debug, -d`: Enable debug/verbose logging
- `--host <HOST>`: Host address to bind to (default: 127.0.0.1)
- `--help, -h`: Show help message

## Running Examples

### Basic Usage (default port 50051)
```bash
cargo run --bin eth-basic
```

### Custom Port
```bash
cargo run --bin eth-basic -- --port 8080
```

### Debug Mode with Verbose Logging
```bash
cargo run --bin eth-basic -- --debug
```

### Custom Configuration
```bash
cargo run --bin eth-basic -- --host 0.0.0.0 --port 9090 --debug
```

## Code Structure

- **Default Handler**: Uses the SDK's built-in `DefaultProcessorV3Handler`
- **Synchronous main()**: No `#[tokio::main]` needed - the server handles runtime creation
- **Built-in CLI**: Automatic command line parsing and logging setup
- **Ethereum focus**: Configured for Ethereum chain processing by default

## Implementation Notes

This example shows the simplest pattern for ProcessorV3 servers:

1. **Minimal code**: Just `Server::new().start()`
2. **No handler implementation**: SDK provides default ProcessorV3Handler
3. **No async setup**: The SDK handles all async runtime management
4. **Built-in logging**: Automatic tracing setup based on CLI args
5. **Zero boilerplate**: Focus on getting started quickly

## Debug Logging

When `--debug` is enabled, you'll see detailed logs including:
- Line numbers and file locations
- Client connection details
- Request/response tracing
- Internal processing steps

## Next Steps

For a production processor, you would implement a custom handler:

```rust
use sentio_sdk::{ProcessorV3Handler, Server};

struct MyCustomProcessor;

#[tonic::async_trait]
impl ProcessorV3Handler for MyCustomProcessor {
    // Implement init, configure_handlers, process_bindings_stream
}

fn main() -> Result<()> {
    let server = Server::with_handler(MyCustomProcessor);
    server.start()
}
```

Key areas to customize:

1. **Configure handlers**: Add specific contract configurations in `configure_handlers`
2. **Implement streaming**: Process real blockchain data in `process_bindings_stream`  
3. **Add business logic**: Generate metrics, events, and database updates
4. **Error handling**: Robust error management and recovery
5. **Testing**: Unit tests for your processor logic

## Custom Handler Example

The SDK provides `DefaultProcessorV3Handler` that you can extend or replace entirely with `Server::with_handler()`.

## Related Documentation

- [Sentio SDK Documentation](../../sdk/README.md)  
- [Protocol Buffer Definitions](../../sdk/processor.proto)