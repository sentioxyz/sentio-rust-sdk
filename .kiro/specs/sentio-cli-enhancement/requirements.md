# Requirements Document

## Introduction

The Sentio CLI is a command-line tool designed to help developers build, manage, and deploy Sentio processors. The tool currently has basic `build` and `init` commands but needs to be enhanced with additional functionality including code generation, binary uploading, and other processor management capabilities. The CLI should provide a comprehensive workflow for processor development from initialization to deployment.

## Requirements

### Requirement 1

**User Story:** As a developer, I want to build my processor for Linux x86_64 target, so that I can create deployable binaries for the Sentio platform.

#### Acceptance Criteria

1. WHEN I run `sentio build` THEN the system SHALL compile the processor project for Linux x86_64 target
2. WHEN I run `sentio build --path <custom_path>` THEN the system SHALL build the processor at the specified path
3. WHEN the build fails THEN the system SHALL display clear error messages with actionable feedback
4. WHEN the build succeeds THEN the system SHALL output the location of the generated binary
5. IF the project is not a valid processor project THEN the system SHALL display an appropriate error message

### Requirement 2

**User Story:** As a developer, I want to generate code for my processor project, so that I can quickly scaffold handlers and boilerplate code automatically.

#### Acceptance Criteria

1. WHEN I run `sentio gen` THEN the system SHALL automatically generate handlers and contract bindings for all contracts in the project
2. WHEN I run `sentio gen --no-handlers` THEN the system SHALL generate only contract bindings without handler templates
3. WHEN I run `sentio gen --no-contracts` THEN the system SHALL generate only handler templates without contract bindings
4. WHEN I run `sentio gen --contract <address>` THEN the system SHALL generate code only for the specified contract
5. WHEN generating code THEN the system SHALL place files in appropriate project directories
6. IF the target directory already contains conflicting files THEN the system SHALL prompt for confirmation before overwriting

### Requirement 3

**User Story:** As a developer, I want to upload my built processor binary to Sentio, so that I can deploy my processor to the platform.

#### Acceptance Criteria

1. WHEN I run `sentio upload` THEN the system SHALL upload the built binary to Sentio platform
2. WHEN I run `sentio upload --binary <path>` THEN the system SHALL upload the specified binary file
3. WHEN uploading THEN the system SHALL authenticate with Sentio platform using API credentials
4. WHEN upload succeeds THEN the system SHALL display confirmation with deployment details
5. IF authentication fails THEN the system SHALL display clear error message with setup instructions
6. IF the binary file doesn't exist THEN the system SHALL display an error and suggest running build first

### Requirement 4

**User Story:** As a developer, I want to initialize new processor projects with templates, so that I can quickly start development with proper project structure.

#### Acceptance Criteria

1. WHEN I run `sentio init <name>` THEN the system SHALL create a new processor project with the given name
2. WHEN initializing THEN the system SHALL create proper directory structure with src/, Cargo.toml, and configuration files
3. WHEN initializing THEN the system SHALL include example handler code and documentation
4. IF a directory with the same name exists THEN the system SHALL prompt for confirmation or suggest alternatives
5. WHEN initialization completes THEN the system SHALL display next steps for the developer

### Requirement 5

**User Story:** As a developer, I want my processor configuration and code to be validated automatically during build, so that I can catch issues before deployment.

#### Acceptance Criteria

1. WHEN I run `sentio build` THEN the system SHALL automatically validate the processor configuration before building
2. WHEN I run `sentio build --no-validate` THEN the system SHALL skip validation and proceed directly to building
3. WHEN validating THEN the system SHALL verify all required dependencies are present
4. WHEN validating THEN the system SHALL check for common code issues and anti-patterns
5. IF validation fails THEN the system SHALL display specific issues with suggestions for fixes and abort the build
6. WHEN validation passes THEN the system SHALL proceed with the build process

### Requirement 6

**User Story:** As a developer, I want to manage my Sentio platform credentials, so that I can authenticate for upload and deployment operations.

#### Acceptance Criteria

1. WHEN I run `sentio auth login` THEN the system SHALL prompt for API credentials and store them securely
2. WHEN I run `sentio auth status` THEN the system SHALL display current authentication status
3. WHEN I run `sentio auth logout` THEN the system SHALL clear stored credentials
4. WHEN storing credentials THEN the system SHALL use secure local storage mechanisms
5. IF credentials are invalid THEN the system SHALL display clear error messages with recovery steps

### Requirement 7

**User Story:** As a developer, I want to add contracts to my processor project, so that I can configure which contracts my processor should monitor and process.

#### Acceptance Criteria

1. WHEN I run `sentio contract add <address>` THEN the system SHALL add the contract to the project configuration
2. WHEN I run `sentio contract add <address> --name <name>` THEN the system SHALL add the contract with a custom name
3. WHEN I run `sentio contract add <address> --network <network>` THEN the system SHALL add the contract for a specific network
4. WHEN adding a contract THEN the system SHALL validate the contract address format
5. WHEN adding a contract THEN the system SHALL fetch and store contract ABI if available
6. IF the contract already exists in the project THEN the system SHALL display a warning and ask for confirmation to update

### Requirement 8

**User Story:** As a developer, I want to run tests for my Sentio processor project, so that I can verify my processor logic works correctly before deployment.

#### Acceptance Criteria

1. WHEN I run `sentio test` THEN the system SHALL execute all tests in the processor project
2. WHEN I run `sentio test --filter <pattern>` THEN the system SHALL run only tests matching the specified pattern
3. WHEN I run `sentio test --release` THEN the system SHALL run tests in release mode for performance testing
4. WHEN running tests THEN the system SHALL display test results with clear pass/fail indicators
5. WHEN tests fail THEN the system SHALL display detailed error information and exit with non-zero code
6. WHEN all tests pass THEN the system SHALL display a success summary and exit with zero code

### Requirement 9

**User Story:** As a developer, I want to see helpful information and documentation, so that I can learn how to use the CLI effectively.

#### Acceptance Criteria

1. WHEN I run `sentio --help` THEN the system SHALL display comprehensive help information for all commands
2. WHEN I run `sentio <command> --help` THEN the system SHALL display detailed help for the specific command
3. WHEN I run `sentio version` THEN the system SHALL display the current CLI version and build information
4. WHEN displaying help THEN the system SHALL include examples and common usage patterns
5. WHEN an invalid command is used THEN the system SHALL suggest similar valid commands