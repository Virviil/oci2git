# Contributing Guidelines

Thank you for your interest in contributing to this project!  
We welcome improvements of all kinds — bug fixes, new features, documentation updates, or test coverage.

Please read these guidelines before opening a Pull Request.

---

## General Rules

1. **Tests are required.**  
   Every pull request that changes code must include tests that demonstrate the correctness of the change.  
   - If you fix a bug, add a test that fails without your fix and passes with it.  
   - If you add a feature, cover its expected behavior with tests.

2. **LLMs and vibe-coding are allowed.**  
   You can use ChatGPT, Claude, Copilot, or simply "vibe-code" however you like.  
   But:  
   - **Always review, clean up, and understand** the code before submitting.  
   - Raw, unreviewed AI output or incoherent dumps will be rejected.

3. **Keep it simple and clean.**  
   - Follow the [KISS principle](https://en.wikipedia.org/wiki/KISS_principle).  
   - Respect [SOLID principles](https://en.wikipedia.org/wiki/SOLID) where applicable.  
   - Prioritize readability, maintainability, and minimalism.

4. **Dependencies are carefully reviewed.**  
   - Adding a new dependency may be subject to strict review.  
   - Prefer small, well-maintained crates with minimal transitive dependencies.  
   - Justify why the dependency is necessary.

---

## Rust-Specific Requirements

- Run **formatting** before committing:  
  ```bash
  cargo fmt --all
  ```

- Run **clippy** and fix warnings:  
  ```bash
  cargo clippy --all-targets --all-features -- -D warnings
  ```

- Run **tests** locally:  
  ```bash
  cargo test
  ```

- If applicable, run **doc tests** and ensure examples build:  
  ```bash
  cargo test --doc
  ```

- CI will enforce these checks. PRs that fail them will not be merged.

---

## How to Contribute

1. **Fork and branch.**  
   - Fork the repository.  
   - Create a new branch for your work:  
     ```bash
     git checkout -b feature/my-change
     ```

2. **Run checks locally.**  
   - `cargo fmt`  
   - `cargo clippy`  
   - `cargo test`  

3. **Open a Pull Request.**  
   - Describe your changes clearly.  
   - Link any related issues.  
   - Keep PRs focused and small when possible.  

---

## Code Style

- Follow the project’s existing style and conventions.  
- Keep commits clean and logically separated.  
- Write meaningful commit messages (imperative style preferred, e.g., *"Add X"* not *"Added X"*).  

---

## Community Standards

- Be respectful and constructive in discussions and code reviews.  
- By contributing, you agree to follow the [Contributor Covenant Code of Conduct](https://www.contributor-covenant.org/).  

---

## What Contributions Are Welcome

- Bug fixes with tests.  
- Small, incremental improvements.  
- Documentation clarifications and examples.  
- New features (preferably discussed in an issue first).  
- Refactoring that improves readability, performance, or reliability.  

---

Happy hacking, and thank you for contributing!
