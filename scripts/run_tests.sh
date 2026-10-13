#!/usr/bin/env bash
set -e

# ==============================================================================
# serchX Test Suite Runner
# Runs all unit, integration, and benchmark tests for serchX.
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
MANIFEST_PATH="${REPO_ROOT}/src-tauri/Cargo.toml"

# Colors for terminal output
if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
  COLOR_RESET="\033[0m"
  COLOR_INFO="\033[1;34m"
  COLOR_SUCCESS="\033[1;32m"
  COLOR_WARN="\033[1;33m"
  COLOR_ERROR="\033[1;31m"
  COLOR_BOLD="\033[1m"
else
  COLOR_RESET=""
  COLOR_INFO=""
  COLOR_SUCCESS=""
  COLOR_WARN=""
  COLOR_ERROR=""
  COLOR_BOLD=""
fi

print_header() {
  echo -e "\n${COLOR_INFO}==>${COLOR_RESET} ${COLOR_BOLD}$1${COLOR_RESET}"
}

print_success() {
  echo -e "${COLOR_SUCCESS}✓ $1${COLOR_RESET}"
}

print_error() {
  echo -e "${COLOR_ERROR}✗ $1${COLOR_RESET}" >&2
}

show_help() {
  cat << EOF
Usage: $(basename "$0") [OPTIONS] [-- CARGO_TEST_ARGS]

serchX test runner script.

OPTIONS:
  -h, --help            Show this help message
  -a, --all             Run format check, clippy linter, and all tests
  -f, --fmt             Run code formatting check (cargo fmt)
  -c, --clippy          Run Clippy lints (cargo clippy)
  -b, --benchmark       Run the serchX-100 retrieval benchmark with --nocapture
  -n, --nocapture       Pass --nocapture to cargo test to view test output in real-time
  -t, --test <NAME>     Run a specific test target (e.g. native_metal_tests)

EXAMPLES:
  # Run all unit and integration tests
  ./scripts/run_tests.sh

  # Run all checks (format + clippy + tests)
  ./scripts/run_tests.sh --all

  # Run a specific test target
  ./scripts/run_tests.sh --test native_metal_tests

  # Run tests with stdout visible
  ./scripts/run_tests.sh --nocapture

  # Run serchX-100 benchmark
  ./scripts/run_tests.sh --benchmark

  # Pass custom arguments to cargo test
  ./scripts/run_tests.sh -- llm_engine
EOF
}

# Ensure cargo is installed
if ! command -v cargo >/dev/null 2>&1; then
  print_error "cargo is not installed or not in PATH."
  exit 1
fi

RUN_FMT=false
RUN_CLIPPY=false
RUN_BENCHMARK=false
SPECIFIC_TEST=""
NOCAPTURE=false
EXTRA_ARGS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help)
      show_help
      exit 0
      ;;
    -a|--all)
      RUN_FMT=true
      RUN_CLIPPY=true
      shift
      ;;
    -f|--fmt)
      RUN_FMT=true
      shift
      ;;
    -c|--clippy)
      RUN_CLIPPY=true
      shift
      ;;
    -b|--benchmark)
      RUN_BENCHMARK=true
      shift
      ;;
    -n|--nocapture)
      NOCAPTURE=true
      shift
      ;;
    -t|--test)
      if [[ -n "${2:-}" ]]; then
        SPECIFIC_TEST="$2"
        shift 2
      else
        print_error "--test requires a test target name."
        exit 1
      fi
      ;;
    --)
      shift
      EXTRA_ARGS+=("$@")
      break
      ;;
    *)
      EXTRA_ARGS+=("$1")
      shift
      ;;
  esac
done

START_TIME=$(date +%s)

# 1. Format check (optional)
if [ "$RUN_FMT" = true ]; then
  print_header "Checking code formatting (cargo fmt)..."
  if cargo fmt --manifest-path "${MANIFEST_PATH}" --all -- --check; then
    print_success "Code formatting check passed."
  else
    print_error "Code formatting check failed. Run 'cargo fmt --manifest-path src-tauri/Cargo.toml' to format."
    exit 1
  fi
fi

# 2. Clippy check (optional)
if [ "$RUN_CLIPPY" = true ]; then
  print_header "Running Clippy lints (cargo clippy)..."
  if cargo clippy --manifest-path "${MANIFEST_PATH}" --all-targets -- -D warnings; then
    print_success "Clippy lints passed."
  else
    print_error "Clippy check failed."
    exit 1
  fi
fi

# 3. serchX-100 benchmark (optional)
if [ "$RUN_BENCHMARK" = true ]; then
  print_header "Running serchX-100 benchmark..."
  cargo test --manifest-path "${MANIFEST_PATH}" --test serchx100_benchmark -- --nocapture
  print_success "serchX-100 benchmark completed."
  exit 0
fi

# 4. Standard tests
print_header "Running serchX tests..."

CARGO_CMD=("cargo" "test" "--manifest-path" "${MANIFEST_PATH}")

if [ -n "$SPECIFIC_TEST" ]; then
  CARGO_CMD+=("--test" "$SPECIFIC_TEST")
fi

if [ ${#EXTRA_ARGS[@]} -gt 0 ]; then
  CARGO_CMD+=("${EXTRA_ARGS[@]}")
fi

if [ "$NOCAPTURE" = true ]; then
  # If extra args didn't already introduce `--`, add it
  has_separator=false
  for arg in "${EXTRA_ARGS[@]}"; do
    if [ "$arg" = "--" ]; then
      has_separator=true
      break
    fi
  done
  if [ "$has_separator" = false ]; then
    CARGO_CMD+=("--")
  fi
  CARGO_CMD+=("--nocapture")
fi

echo "Executing: ${CARGO_CMD[*]}"
"${CARGO_CMD[@]}"

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

print_header "Test Results"
print_success "All serchX tests passed successfully in ${DURATION}s!"
