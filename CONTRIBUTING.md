# Contributing to Waymesh

Thank you for your interest in contributing to Waymesh, an open protocol for decentralized mobility and community logistics coordination.

## Development Workflow

1. **Fork & Branch:**
   * Create a topic branch from `main` or `dev`.
   * Branch naming: `feature/short-description` or `fix/issue-description`.

2. **Code Quality & Rust Conventions:**
   * Run the test suite:
     ```bash
     cargo test --workspace
     ```
   * Ensure zero Clippy warnings:
     ```bash
     cargo clippy --workspace --all-targets -- -D warnings
     ```
   * Format code with rustfmt:
     ```bash
     cargo fmt --check
     ```

3. **Core Protocol Principles:**
   * **Decoupled Solvers:** The optimization engine must remain decoupled behind the `MatchingSolver` trait.
   * **Strict Gender Separation:** In passenger rides, cross-gender matching between male and female participants is strictly prohibited across all layers (solvers and API gates). Parcel crowdshipping is gender-agnostic.
   * **Private Direct Settlements:** Waymesh does not accept speculative cryptocurrencies, tokens, or blockchain ledgers. All cost-shares are settled directly between parties privately (via sovereign cash, direct peer fiat like UPI, or private mutual agreement).

4. **Pull Requests:**
   * Provide a concise description of your changes and reference any related issues.
   * Include unit or integration tests for all new behaviors.
