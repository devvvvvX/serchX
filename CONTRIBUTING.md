# Contributing to serchX

Thank you for your interest in contributing to **serchX**! We are building a private, local-first file memory layer for macOS — finding what you can't remember across time, content, screenshots, and version history without ever sending data to the cloud.

Whether you are fixing a bug, adding an extractor for a new file format, optimizing search index latency, or refining the macOS UI, we welcome your contributions.

---

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Our Core Principles](#our-core-principles)
- [Development Setup](#development-setup)
  - [Prerequisites](#prerequisites)
  - [Setting Up the Environment](#setting-up-the-environment)
  - [Running in Development Mode](#running-in-development-mode)
  - [Building Release Bundles](#building-release-bundles)
- [Project Architecture](#project-architecture)
  - [Directory Structure](#directory-structure)
  - [Backend (Rust & Tauri 2.x)](#backend-rust--tauri-2x)
  - [Frontend (HTML5, Vanilla CSS, ES6 JS)](#frontend-html5-vanilla-css-es6-js)
- [Development Workflow](#development-workflow)
  - [1. Branching Strategy](#1-branching-strategy)
  - [2. Code Formatting & Linting](#2-code-formatting--linting)
  - [3. Running Tests & Benchmarks](#3-running-tests--benchmarks)
  - [4. Commit Message Conventions](#4-commit-message-conventions)
  - [5. Submitting a Pull Request](#5-submitting-a-pull-request)
- [Architectural & Coding Guidelines](#architectural--coding-guidelines)
  - [Zero-Cloud Privacy & Security](#zero-cloud-privacy--security)
  - [Performance & Resource Budgets](#performance--resource-budgets)
  - [Resilience & Graceful Fallbacks](#resilience--graceful-fallbacks)
  - [macOS Human Interface Guidelines (HIG)](#macos-human-interface-guidelines-hig)
- [Issue Reporting & Feature Requests](#issue-reporting--feature-requests)
- [License](#license)

---

## Code of Conduct

serchX is committed to providing a welcoming, inclusive, and harassment-free environment for everyone. All contributors and participants are expected to adhere to our [Code of Conduct](CODE_OF_CONDUCT.md).

Please review the full [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for detailed community standards, reporting procedures, and enforcement guidelines.

---

## Our Core Principles

Before writing code for serchX, please keep these non-negotiable architectural tenets in mind:

1. **100% Offline & Local-First**: No remote analytics, no telemetry, no third-party cloud embeddings or inference endpoints. Everything must execute on the user's local machine.
2. **Deterministic & Transparent**: Search rankings and AI summaries must provide verifiable evidence ("Why this result" cues, citations, and source file snippets).
3. **Respect System Resources**: macOS users value battery life and quiet fans. serchX enforces strict idle RAM caps (<200MB RSS), 5-minute memory TTL for on-device neural models, and thermal/battery throttling.
4. **Native macOS Polish**: Follow Apple Human Interface Guidelines (HIG) closely. We favor native vibrancy, SF-style typography, keyboard navigation, and fluid transitions over heavy multi-framework abstractions.

---

## Development Setup

### Prerequisites

- **Operating System**: macOS 13.0 (Ventura) or newer (Apple Silicon M-Series or Intel x86_64).
- **Xcode Command Line Tools**:
  ```bash
  xcode-select --install
  ```
- **Rust Toolchain** (1.82+ with 2021 edition):
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  rustup update stable
  ```
- **Tauri CLI v2**:
  ```bash
  cargo install tauri-cli --version "^2.0"
  ```
- **Node.js** (v18+, optional for alternative CLI runners).

### Setting Up the Environment

1. Fork the repository on GitHub: [devvvvvX/serchX](https://github.com/devvvvvX/serchX).
2. Clone your fork locally:
   ```bash
   git clone https://github.com/<your-username>/serchX.git
   cd serchX
   ```
3. Verify your Rust and Tauri toolchain:
   ```bash
   rustc --version
   cargo --version
   cargo tauri --version
   ```

### Running in Development Mode

Run the app with live reload enabled:

```bash
cargo tauri dev
```

This launches the native macOS app with the Spotlight floating launcher (`index.html`) listening for invocation hotkeys (<kbd>Cmd</kbd> + <kbd>Shift</kbd> + <kbd>Space</kbd>).

### Building Release Bundles

To build the standalone `.app` bundle:

```bash
cargo tauri build --bundles app
```
The compiled bundle will be available at:
`src-tauri/target/release/bundle/macos/serchX.app`

To package a distributable `.dmg` installer:

```bash
./scripts/build_dmg.sh
```

---

## Project Architecture

### Directory Structure

```text
serchX/
├── scripts/                # Packaging, DMG generation & utility scripts
│   ├── build_dmg.sh        # Automates release compilation & UDZO DMG creation
│   └── gen_icon.py         # Multi-resolution icon set generation helper
├── src-tauri/              # Rust Native Backend (Tauri 2.x)
│   ├── src/
│   │   ├── commands/       # Tauri IPC endpoints exposed to the UI
│   │   ├── crawler/        # File scanner, CrawlCoordinator, and DeleteQueue worker
│   │   ├── db/             # SQLite 3 schema, WAL pool, batch purges, and vacuuming
│   │   ├── diff/           # Myers/LCS text comparison engine & LRU diff cache
│   │   ├── ingest/         # File parsers (PDFKit fallback, DOCX, TXT, MD, Code) + Vision OCR
│   │   ├── llm/            # Native Metal GGUF inference, LM Studio discovery & catalog
│   │   ├── models/         # Data contracts (Artifact, Version, Chunk, Folder, DeadLetter)
│   │   ├── safety/         # Streaming SHA-256, path permissions, thermal/power checks
│   │   ├── search/         # SQLite FTS5 (BM25), temporal parsing & RRF hybrid ranking
│   │   └── vector/         # Fastembed local embeddings & cosine vector index
│   ├── tests/              # Comprehensive integration tests & serchX-100 benchmark
│   ├── Cargo.toml          # Rust dependencies & crate configuration
│   └── tauri.conf.json     # Window definitions, capabilities & bundle metadata
└── ui/                     # Lightweight HTML5 / Vanilla CSS / ES6 Frontend
    ├── index.html          # Spotlight launcher floating window
    ├── app.js              # Launcher logic, keyboard navigation & synthesis rendering
    ├── settings.html       # Multi-tier settings, telemetry & source management
    ├── settings.js         # Settings controller, delete queue & queue drawer
    ├── compare.html        # Dedicated standalone diff inspection window
    ├── compare.js          # Unified & split diff viewer controller
    ├── onboarding.html     # Guided multi-step first-run wizard
    ├── onboarding.js       # Permission verification & model setup
    └── styles/             # Modular CSS design system (tokens, spotlight, settings, etc.)
```

### Backend (Rust & Tauri 2.x)

- **Language & Runtime**: Modern Rust (2021 edition), Tokio async runtime.
- **Storage**: SQLite in WAL (Write-Ahead Logging) mode via `rusqlite` with FTS5 enabled.
- **Inference**: Native Metal GPU-accelerated GGUF execution via `llama-cpp-2`.
- **Extraction**: Dual-layer architecture: Rust native parsers (`pdf-extract`, `docx-rs`) backed by macOS native `PDFKit` fallback and Apple Vision framework for OCR.
- **IPC Throttling**: Event channels stream with a 150ms throttle buffer to prevent WebKit UI thread saturation during heavy crawl cycles.

### Frontend (HTML5, Vanilla CSS, ES6 JS)

- **No Heavy Framework Overhead**: serchX does not use React, Vue, or Angular. This ensures near-zero startup delay, minimal memory footprint, and instantaneous window activation.
- **Styling**: Pure CSS organized around CSS custom properties in `ui/styles/tokens.css` providing dark/light palette harmony, macOS vibrancy filters, and typography adhering to San Francisco font metrics.
- **Markdown**: Grounded synthesis responses in `app.js` utilize `marked.js` with Apple-styled code blocks, citations, and tables.

---

## Development Workflow

### 1. Branching Strategy

- Always create your feature or bug fix branch from `main`:
  ```bash
  git checkout -b feature/your-feature-name
  # or
  git checkout -b fix/issue-description
  ```
- Keep branches focused and scoped to a single purpose.

### 2. Code Formatting & Linting

Before pushing your changes, ensure your code satisfies formatting and clippy rules:

```bash
# Format Rust code
cargo fmt --all --manifest-path src-tauri/Cargo.toml

# Check formatting without modifying
cargo fmt --all --manifest-path src-tauri/Cargo.toml -- --check

# Run Clippy lints
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

For JavaScript and CSS in `ui/`:
- Use clean, modern ES6+ JavaScript.
- Avoid external npm dependencies unless explicitly discussed and approved.
- Preserve consistent 2-space indentation and clear commenting on complex UI logic.

### 3. Running Tests & Benchmarks

serchX maintains an extensive test suite covering indexing pipelines, parser fallbacks, FTS5 latency, and differential diffs:

```bash
# Convenient runner script for all tests
./scripts/run_tests.sh

# Or directly with Cargo
cargo test --manifest-path src-tauri/Cargo.toml

# Run a specific test suite
./scripts/run_tests.sh --test folder_analysis_test
cargo test --manifest-path src-tauri/Cargo.toml --test indexing_queue_test
cargo test --manifest-path src-tauri/Cargo.toml --test llm_engine_test
```

#### Running the `serchX-100` Retrieval Benchmark

Whenever modifying search, scoring, rank fusion, or temporal parsing logic, run the 100-query benchmark to verify that recall and latency targets are preserved:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test serchx100_benchmark -- --nocapture
```

Ensure your changes meet the baseline performance targets:
- Exact term query latency (p95): `< 50 ms`
- Hybrid search latency (p95): `< 200 ms`
- Temporal query parser latency: `< 5 ms`
- Idle physical RAM: `< 200 MB RSS`

### 4. Commit Message Conventions

We adhere to [Conventional Commits](https://www.conventionalcommits.org/):

```text
<type>(<scope>): <short description>

[optional body explaining rationale]
```

**Common Types:**
- `feat`: A new user-facing feature or extraction capability.
- `fix`: A bug fix or error mitigation.
- `perf`: A code change that improves indexing speed or memory footprint.
- `refactor`: Code reorganization without functional changes.
- `test`: Adding or improving unit, integration, or benchmark tests.
- `docs`: Documentation updates or clarifications.
- `chore`: Tooling, dependency, or build script updates.

**Examples:**
- `feat(ingest): add native epub text extractor with chapter chunking`
- `fix(crawler): prevent deadlocks during high-concurrency folder cancellation`
- `perf(db): optimize batch FTS5 deletion using prepared statements`
- `docs: update keyboard shortcuts table in README`

### 5. Submitting a Pull Request

1. **Push your branch** to your fork:
   ```bash
   git push origin feature/your-feature-name
   ```
2. **Open a Pull Request** against the `main` branch of `devvvvvX/serchX`.
3. **Fill out the PR description** clearly explaining:
   - What changed and why.
   - Any architectural tradeoffs made.
   - How you tested the changes (include test output or benchmark numbers if applicable).
   - Screenshots or video clips for UI changes.
4. **Respond to review feedback** promptly. We will gladly collaborate to get your PR merged!

---

## Architectural & Coding Guidelines

### Zero-Cloud Privacy & Security

- **Strict Local Boundary**: Under no circumstances should serchX send data to an external server. PRs containing remote analytics, telemetry beacons, or external AI API calls will be rejected.
- **Safety Filters**: All ingested paths must pass through `src/safety/` validation. System critical directories (`/System`, `/Library`, `/private`), sensitive configuration files (`.ssh`, `.gnupg`, `.aws`, `.env`), and serchX's own internal application data directories (`~/Library/Application Support/com.serchx.desktop`) must remain strictly excluded.
- **Streaming Hashes**: Always hash file contents using chunked streaming buffers (`sha2::Sha256`) rather than loading entire large files into contiguous memory at once.

### Performance & Resource Budgets

- **Non-Blocking IO**: Never perform heavy filesystem scans, SQLite transactions, or ML model loads on the main thread. Always dispatch to Tokio worker threads or dedicated background queues (`CrawlCoordinator`, `DeleteQueue`).
- **Throttled IPC Channels**: UI progress updates must be rate-limited (100–150ms buffer) to avoid triggering IPC backpressure and freezing the macOS WebKit view.
- **Memory Lifecycle**: On-device neural models must implement an idle time-to-live (5-minute TTL) so memory is promptly freed when search synthesis finishes.
- **Background Maintenance**: After large deletions or purge events, execute chunked cleanup queries and trigger SQLite `VACUUM` in the background with progress signaling.

### Resilience & Graceful Fallbacks

- **Extractors Must Fail Gracefully**: A corrupted PDF or unsupported binary file must never crash the crawler. Use fallback tiers (e.g. `lopdf` $\to$ `PDFKit`), and record persistent failures in the `DeadLetterQueue` so they can be inspected and debugged.
- **Cancellation Awareness**: Long-running crawl or purge operations must check `AtomicBool` cancellation flags periodically to ensure immediate responsiveness when a user pauses or cancels an action.

### macOS Human Interface Guidelines (HIG)

- **Keyboard First**: Every primary interaction should be accessible via intuitive keyboard shortcuts (`Esc` to dismiss, `Cmd+Enter` to reveal in Finder, `Cmd+D` for diff, `Cmd+P` to pin version).
- **Native Look & Feel**: Preserve native window styling (`Overlay` title bars, standard traffic light positioning, HUD window effects, and dynamic vibrancy).
- **Subtle Micro-Interactions**: Use smooth, hardware-accelerated CSS transitions. Avoid harsh, unstyled layout shifts.

---

## Issue Reporting & Feature Requests

### Reporting Bugs

If you discover a bug, please check the [existing issues](https://github.com/devvvvvX/serchX/issues) first. If it has not been reported, open a new issue with:
1. **macOS Version** (e.g. macOS 14.5 Sonoma) and architecture (Apple Silicon M1/M2/M3/M4 or Intel).
2. **Steps to Reproduce**: Clear, reproducible sequence of events.
3. **Expected vs. Actual Behavior**.
4. **Relevant Log Output**: Located at `~/Library/Logs/com.serchx.desktop/` or in the Settings Diagnostics tab.

### Suggesting Features

We welcome ideas that make serchX faster, more intuitive, or capable of understanding more file types. Please open an issue labeled `enhancement` describing:
- The problem you are trying to solve.
- Your proposed solution or workflow.
- Why a local-first approach is well-suited for this feature.

---

## License

By contributing to **serchX**, you agree that your contributions will be licensed under the [MIT License](LICENSE).
