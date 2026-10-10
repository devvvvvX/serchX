# serchX

> **Private, local-first file memory layer for macOS.**  
> Find what you can't remember — across time, content, screenshots, and version history.

[![Release](https://img.shields.io/badge/Release-v1.0.0-blue?style=flat-square)](https://github.com/devvvvvX/serchX/releases)
[![macOS](https://img.shields.io/badge/macOS-Apple%20Silicon%20%7C%20Intel-black?style=flat-square&logo=apple)](https://apple.com)
[![Rust](https://img.shields.io/badge/Rust-Tauri%202.x-orange?style=flat-square&logo=rust)](https://tauri.app)
[![SQLite](https://img.shields.io/badge/SQLite-FTS5%20WAL-003B57?style=flat-square&logo=sqlite)](https://sqlite.org)
[![Neural](https://img.shields.io/badge/Neural-Apple%20Metal%20GGUF-purple?style=flat-square)](#3-grounded-on-device-neural-synthesis--markedjs-markdown)
[![Privacy](https://img.shields.io/badge/Privacy-100%25%20Local%20%7C%20Zero%20Cloud-success?style=flat-square)](#privacy--security-posture)
[![Tests](https://img.shields.io/badge/Tests-Passing-brightgreen?style=flat-square)](#testing--benchmarks)

---

## What is serchX?

**serchX is not a generic AI search engine. It is a private memory layer for files on your Mac.**

Spotlight and Siri AI are built for system actions, app launching, and cloud-assisted assistant queries. But when you need to recall a specific local document using human memory cues, system search falls short:
- *"The contract before pricing changed"*
- *"The spreadsheet I edited last Tuesday"*
- *"The screenshot with the Docker error from yesterday"*
- *"The proposal file in Downloads from June"*

serchX solves this by indexing your chosen folders into a fast, transparent local SQLite engine with **version snapshots, Apple Vision OCR, reciprocal rank fusion (RRF) hybrid search, explainable match evidence, and native on-device neural synthesis**.

```text
┌────────────────────────────────────────────────────────────────────────┐
│  🔍 Find the contract before pricing changed                           │
├────────────────────────────────────────────────────────────────────────┤
│  📄 Acme MSA v3.pdf                         Jun 18 • ~/Documents/Acme/ │
│                                                                        │
│  ✨ Why this result:                                                   │
│     • Saved before pricing clause revision on Jun 21                   │
│     • Matched phrases: "payment terms", "net 45", "tier discount"      │
│     • Modified: Jun 18, 2026 (2 versions recorded)                    │
│                                                                        │
│  [Open File]   [Reveal in Finder]   [Compare Versions]   [📌 Pin]      │
└────────────────────────────────────────────────────────────────────────┘
```

---

## Key Capabilities

### 1. Version-Aware Memory & Standalone Diff Window
- **Automatic Version Snapshots**: Tracks content modifications as versions without modifying your filesystem.
- **Dedicated Compare Window**: Independent native macOS window with traffic light controls, version dropdown pickers, and live path discovery (`get_versions_for_path`).
- **LCS Unified & Split Diffs**: Inspect exactly what changed between any two snapshots with line-by-line syntax coloring, additions/deletions counts, and whitespace-normalized formatting.
- **Version Pinning**: Protect crucial milestones against automated retention pruning.
- **Directional Queries**: Natural queries like *"before pricing changed"* or *"earlier version of pitch deck"* match the exact chronological state.

### 2. Explainable "Why This Result" Evidence
- No opaque black-box AI scores. Every search card surfaces concrete match evidence:
  - **FTS5 Exact Term Matches** (highlighted snippets)
  - **Semantic Vector Similarity** (cosine distance ranking)
  - **Temporal Match Cues** (*yesterday*, *last Tuesday*, *before June*)
  - **OCR Recognition Badges** (extracted text from screenshots/images)
  - **Version Relationships** (*Current*, *Previous Version*, *Modified post-edit*)

### 3. Grounded On-Device Neural Synthesis & Marked.js Markdown
- **Native GGUF Tensor Inference**: Built-in Apple Silicon Metal GPU acceleration powered by `llama-cpp-2` — runs <=4B models locally with zero cloud dependencies or external daemons.
- **Curated Model Catalog**: Includes **OCC-RAG (0.8B default)**, **Qwen 3.5 (0.8B & 2B)**, and **Qwen 3.8 (4B Distill)**, with instant download pause/resume.
- **LM Studio Local Discovery**: Automatically discovers local GGUF models in LM Studio directories (`~/.cache/lm-studio/models` and `~/.lmstudio/models`) with home directory expansion.
- **Rich Synthesis Rendering**: Spotlight synthesis responses rendered via `marked.js` with Apple-native typography for bold text, lists, code blocks, citations `[1]`, `[2]`, and blockquotes.
- **Graceful Onboarding & Prompts**: Friendly modal guidance to download a model or configure LM Studio/Ollama when synthesis is triggered without a local model.
- **5-Minute Idle TTL Memory Lifecycle**: Models automatically unload from unified memory after 5 minutes of inactivity to protect system RAM on 8 GB and 16 GB Macs.

### 4. Pre-Index Folder Analysis & Light Scans
- **Instant Pre-Crawl Preview**: Run a fast, non-blocking light scan (`analyze_folder_light`) on any folder before adding it to watched sources.
- **Detailed Estimation Modal**: Inspect discovered file counts, total size, file type distribution, and estimated indexing ETA before confirming.
- **Per-Folder Crawler Controls**: Independently pause and resume crawling per watched folder, in addition to global footer pause/resume controls.

### 5. Asynchronous Delete Queue & Background Database Vacuuming
- **Non-Blocking Purging**: Removing a folder or updating blacklists dispatches an asynchronous `DeleteQueue` background worker, preventing UI stalls during large purges.
- **Chunked Deletions & Background Vacuum**: Deletions are processed in optimized SQL chunks followed by a background SQLite `VACUUM` to reclaim disk space with live UI synchronization.
- **Inherited Blacklists & Override Exceptions**: Block parent directories while selectively allowing nested child folders via explicit override badges.
- **Self-Referential Safety**: Automatically excludes the application's own data directory and log files to eliminate infinite indexing loops.

### 6. Resilient Ingestion, Native PDFKit Fallback & Dead Letter Queue
- **Apple Native PDFKit Fallback**: Automatically invokes macOS `PDFKit` when `lopdf` encounters complex, encrypted, or corrupted PDFs, ensuring high extraction success.
- **Inspectable Dead Letter Queue**: Failed extraction tasks are recorded in a Dead Letter Queue (`DeadLetterItem`) with IPC inspection commands, allowing the crawler to continue uninterrupted.

### 7. Real-Time Indexing Queue & Telemetry Dashboard
- **Active File Ticker**: Real-time indexing status showing the exact processing file path and byte size.
- **Inspectable Queue Drawer**: View all pending folders with live progress bars and one-click cancellation.
- **Darwin Kernel RSS Memory Tracking**: Live physical RAM metrics via Darwin `mach_task_basic_info`, alongside Apple Silicon Metal unified memory and KV cache tooltips.
- **Extension & Storage Breakdown**: Live cards showing indexed documents, code files, spreadsheets, OCR images, and disk storage consumption.

### 8. macOS-Native Windowing, Vibrancy & HIG Polish
- **Spotlight-Style Floating Launcher**: Ultra-fast invocation (`Cmd+Shift+Space`) with native vibrancy and dynamic height expansion.
- **Freehand Draggable**: Click and drag the launcher anywhere across multiple displays with position persistence.
- **Native File Dialogs**: Built-in folder pickers powered by `tauri-plugin-dialog`.
- **macOS HIG Button Styling**: Redesigned Settings controls adhering to Apple Human Interface Guidelines with SF-style iconography.
- **Guided Multi-Step Onboarding**: Smooth first-run walkthrough wizard (`onboarding.html`) with permission verification and model discovery.
- **Native App Icons**: Updated high-resolution icons across `.icns`, `.ico`, `.png`, and multiple retina densities.
- **Clean Desktop Feel**: Disabled browser right-click context menu across all app windows.

---

## Architecture & Technology Stack

```text
serchX/
├── ui/                     # Lightweight HTML5 / Vanilla CSS / ES6 Frontend
│   ├── index.html          # Spotlight floating launcher UI with marked.js synthesis
│   ├── app.js              # Launcher controller, keyboard hotkeys & preview pane
│   ├── settings.html       # Dedicated Multi-Tier Settings, models, analytics & diagnostics
│   ├── settings.js         # Queue drawer, blacklist manager, model manager, delete queue & telemetry
│   ├── compare.html        # Dedicated Compare Versions window with native traffic lights
│   ├── compare.js          # Standalone diff controller, version pickers & split/unified views
│   ├── onboarding.html     # Guided multi-step first-run setup wizard
│   ├── onboarding.js       # Permission checks, folder discovery & LM Studio model setup
│   └── styles/             # Modular CSS design system (tokens, base, compare, settings, onboarding, spotlight)
└── src-tauri/              # Rust Native Backend (Tauri 2.x)
    ├── src/
    │   ├── commands/       # Tauri IPC endpoints (crawler, diff, llm, onboarding, search, settings, ui, version)
    │   ├── crawler/        # Multi-threaded file crawler, CrawlCoordinator, DeleteQueue & queue tracking
    │   ├── db/             # SQLite connection pool, WAL mode schema, chunked purge & vacuum
    │   ├── diff/           # Myers / LCS text diff engine & LRU diff cache
    │   ├── ingest/         # Text extractors (PDFKit fallback, DOCX, TXT, MD, Code) + Vision OCR
    │   ├── llm/            # Native Metal GGUF engine, model catalog (OCC-RAG, Qwen 3.5/3.8), LM Studio discovery, downloader, context builder
    │   ├── models/         # Core types (Artifact, Version, Chunk, Folder, Event, Settings, DeadLetter)
    │   ├── safety/         # Streaming SHA-256, thermal/power checks, path permissions, app data directory exclusion
    │   ├── search/         # FTS5 BM25 search, query temporal parser & hybrid RRF
    │   └── vector/         # Local embedder scheduler & SQLite vector index
```

| Component | Technology | Rationale |
| :--- | :--- | :--- |
| **App Framework** | Tauri 2.x + Rust | Native macOS performance, <45MB bundle size, minimal idle RAM |
| **Native Dialogs** | tauri-plugin-dialog | Native macOS open file and folder selection picker |
| **Database** | SQLite 3 (WAL Mode) | Zero external daemon, chunked batch purging, background vacuuming |
| **Text Search** | SQLite FTS5 (BM25) | Sub-10ms exact phrase retrieval, prefix matching, and token scoring |
| **Vector Engine** | Local Vector Index (Cosine) | RRF (Reciprocal Rank Fusion) hybrid scoring, hard corpus caps |
| **Neural Inference** | llama-cpp-2 + Apple Metal | Real on-device GGUF tensor execution with Metal GPU acceleration and 5-min idle memory TTL |
| **Markdown Rendering**| marked.js | Client-side rich Markdown formatting for grounded synthesis responses |
| **PDF Extraction** | lopdf + Apple PDFKit Fallback | Dual-tier extraction guaranteeing parsing success for complex or corrupted PDFs |
| **Image OCR** | Apple Vision Framework | Native on-device OCR for screenshots/scans without cloud dependencies |
| **Process Telemetry** | Darwin mach_task_basic_info | Accurate physical RSS memory metrics and Metal unified memory tooltips |
| **File Watcher** | Notify-rs (FSEvents) | Real-time filesystem change capture (create, modify, rename, delete) |
| **IPC Bridge** | Tauri Channels (150ms throttle) | High-performance throttled streaming preventing WebKit UI freezes |

---

## Privacy & Security Posture

serchX is built on strict local-first principles:

- **100% Offline & Local**: Zero telemetry, zero analytics tracking, zero network pings.
- **No Cloud Embeddings or AI**: Vector embeddings and neural synthesis run exclusively on Apple Silicon CoreML / Metal GPU / CPU.
- **No Account Required**: No logins, license servers, or external services.
- **Respects Sandboxes**: Never touches directories outside of explicitly monitored watched folders.
- **Battery & Thermal Throttling**: Automatically throttles crawling when macOS detects thermal pressure or low battery.

---

## Keyboard Shortcuts

| Shortcut | Action | Scope |
| :--- | :--- | :--- |
| <kbd>Cmd</kbd> + <kbd>Shift</kbd> + <kbd>Space</kbd> | Toggle Spotlight Launcher | Global System-Wide |
| <kbd>Esc</kbd> | Hide Launcher / Close Compare Window | In-App |
| <kbd>Cmd</kbd> + <kbd>,</kbd> | Open Settings & Watched Sources Window | In-App |
| <kbd>↑</kbd> / <kbd>↓</kbd> | Navigate Search Results | In-App Launcher |
| <kbd>Enter</kbd> | Open Selected File in Default App | In-App Launcher |
| <kbd>Cmd</kbd> + <kbd>Enter</kbd> | Reveal Selected File in macOS Finder | In-App Launcher |
| <kbd>Cmd</kbd> + <kbd>D</kbd> | Open Dedicated Compare Versions Window | Selected Result |
| <kbd>Cmd</kbd> + <kbd>P</kbd> | Toggle Version Pin | Selected Result |

---

## Getting Started

### Prerequisites

- macOS 13.0 (Ventura) or newer (Apple Silicon M-Series or Intel)
- [Rust](https://rustup.rs/) (1.78+)
- Xcode Command Line Tools (`xcode-select --install`)
- Node.js (v18+) for Tauri CLI tooling

### Installation & Development

1. **Clone the repository**:
   ```bash
   git clone https://github.com/devvvvvX/serchX.git
   cd serchX
   ```

2. **Run in development mode**:
   ```bash
   # Starts Vite/Tauri development runner with hot reload
   cargo tauri dev
   ```

3. **Build the production macOS application**:
   ```bash
   # Compiles optimized Rust binary and creates macOS .app bundle
   cargo tauri build --bundles app
   ```
   The built `.app` will be located at:
   ```text
   src-tauri/target/release/bundle/macos/serchX.app
   ```

4. **Launch the built app**:
   ```bash
   open src-tauri/target/release/bundle/macos/serchX.app
   ```

---

## Testing & Benchmarks

serchX maintains a strict test suite covering safety filters, streaming hashing, FTS5 latency, RRF hybrid ranking, and crawl throughput:

```bash
# Run all 125 unit, integration, and benchmark tests
cargo test
```

### Automated Benchmark Suite (`serchX-100`)

The project includes the standardized `serchX-100` recall benchmark testing 100 complex retrieval queries (exact, temporal, OCR, and version-differential):

```bash
cargo test --test serchx100_benchmark
```

| Metric | Measured Result | Target | Status |
| :--- | :--- | :--- | :--- |
| **Exact Term Latency (p95)** | **4.2 ms** | < 50 ms | ✅ Pass |
| **Hybrid Search Latency (p95)** | **38.6 ms** | < 200 ms | ✅ Pass |
| **Temporal Query Parse Latency** | **0.8 ms** | < 5 ms | ✅ Pass |
| **Memory Consumption (Idle)** | **136.6 MB RSS** | < 200 MB | ✅ Pass |
| **Idle CPU Utilization** | **< 0.1%** | < 1.0% | ✅ Pass |

---

## Contributing

Contributions are welcome! Please review [CONTRIBUTING.md](CONTRIBUTING.md) for our code of conduct, development setup, benchmark workflows, and architectural guidelines.

---

## License

MIT License. See [LICENSE](LICENSE) for details.
