#!/usr/bin/env bash
# Benchmark run-staged against other staged-file runners. See bench/README.md.
set -euo pipefail

BENCH_DIR=$(cd "$(dirname "$0")" && pwd)
ROOT=$(dirname "$BENCH_DIR")
RESULTS="$BENCH_DIR/results"
TMP=${TMPDIR:-/tmp}
WORK=${TMP%/}/run-staged-bench
# Set to benchmark a prebuilt binary instead of building the workspace.
RUN_STAGED_BIN=${RUN_STAGED_BIN:-}

ALL_TOOLS=run-staged,lint-staged,nano-staged,lefthook,pre-commit,prek,hk,stagelint,overcommit
ALL_SCENARIOS=overhead,scaling,concurrency,partial-staging
TOOLS=$ALL_TOOLS
SCENARIOS=$ALL_SCENARIOS
FILES=10,100,1000,10000
RUNS=10
VERIFY=0
WARMUP=3
DEFAULT_N=100 # file count for every scenario except scaling

NOOP=true
# Ignores the file args each tool appends; runs without a shell for tools that exec directly.
SLEEP="sh -c 'sleep 0.5' sh"
# Appends every file argument that still contains unstaged edits to $WORK/probe.txt.
PROBE=$WORK/probe.sh
# nano-staged cannot exec an absolute path directly, so go through sh.
PROBE_CMD="sh $PROBE"
EXTS=(js ts css md)

usage() {
  cat <<EOF
usage: bench/run.sh [--tools a,b] [--scenarios a,b] [--files 10,100] [--runs N] [--warmup N] [--verify]
  --verify   run each tool once per scenario and report exit status instead of benchmarking
  tools:     $ALL_TOOLS
  scenarios: $ALL_SCENARIOS
EOF
}

while [[ $# -gt 0 ]]; do
  case $1 in
    --tools) TOOLS=$2; shift 2 ;;
    --scenarios) SCENARIOS=$2; shift 2 ;;
    --files) FILES=$2; shift 2 ;;
    --runs) RUNS=$2; shift 2 ;;
    --warmup) WARMUP=$2; shift 2 ;;
    --verify) VERIFY=1; shift ;;
    -h | --help) usage; exit 0 ;;
    *) usage >&2; exit 2 ;;
  esac
done

for bin in hyperfine jq git rsync; do
  command -v "$bin" >/dev/null || { echo "error: $bin is required but not found on PATH" >&2; exit 1; }
done

# --- tools -------------------------------------------------------------------

# Prints the argv (one string, shell-words) hyperfine runs for a tool, or fails if not installed.
tool_cmd() {
  local bin=$BENCH_DIR/node_modules/.bin
  case $1 in
    run-staged) echo "${RUN_STAGED_BIN:-$ROOT/target/release/run-staged} --quiet" ;;
    lint-staged) [[ -x $bin/lint-staged ]] && echo "$bin/lint-staged --quiet" ;;
    nano-staged) [[ -x $bin/nano-staged ]] && echo "$bin/nano-staged" ;;
    lefthook)
      if command -v lefthook >/dev/null; then echo "$(command -v lefthook) run pre-commit"
      elif [[ -x $bin/lefthook ]]; then echo "$bin/lefthook run pre-commit"
      else return 1; fi ;;
    pre-commit) command -v pre-commit >/dev/null && echo "$(command -v pre-commit) run" ;;
    prek) command -v prek >/dev/null && echo "$(command -v prek) run" ;;
    hk) command -v hk >/dev/null && echo "$(command -v hk) run pre-commit" ;;
    stagelint) command -v stagelint ;;
    overcommit) command -v overcommit >/dev/null && echo ".git/hooks/pre-commit" ;;
    *) return 1 ;;
  esac
}

tool_version() {
  local cmd
  cmd=$(tool_cmd "$1")
  case $1 in
    lefthook) ${cmd% run pre-commit} version ;;
    overcommit) overcommit --version ;;
    pre-commit | prek) ${cmd% run} --version ;;
    hk) ${cmd% run pre-commit} --version ;;
    *) ${cmd% --quiet} --version ;;
  esac 2>&1 | grep -v '^Ignoring' | head -1
}

# --- configs -----------------------------------------------------------------
# Every config is committed into the fixture's base commit (pre-commit refuses unstaged configs).
# Tasks are given as "glob<TAB>command" lines on stdin.

write_configs() {
  local dir=$1 tasks glob cmd i
  tasks=$(cat)

  { # run-staged: root group, parallel by default
    echo "[patterns]"
    while IFS=$'\t' read -r glob cmd; do printf '"%s" = ["%s"]\n' "$glob" "$cmd"; done <<<"$tasks"
  } >"$dir/.run-staged.toml"

  # lint-staged / nano-staged: flat glob -> command map
  jq -Rn '[inputs | split("\t") | {(.[0]): .[1]}] | add' <<<"$tasks" >"$dir/.lintstagedrc.json"
  cp "$dir/.lintstagedrc.json" "$dir/.nano-staged.json"
  cp "$dir/.lintstagedrc.json" "$dir/.stagelint.json"

  { # lefthook
    printf 'pre-commit:\n  parallel: true\n  commands:\n'
    i=0
    while IFS=$'\t' read -r glob cmd; do
      printf '    t%d:\n      glob: "%s"\n      run: %s {staged_files}\n' $i "$glob" "$cmd"
      i=$((i + 1))
    done <<<"$tasks"
  } >"$dir/lefthook.yml"

  { # pre-commit + prek (same file); equal priority lets prek run hooks concurrently
    printf 'repos:\n  - repo: local\n    hooks:\n'
    i=0
    while IFS=$'\t' read -r glob cmd; do
      printf '      - id: t%d\n        name: t%d\n        entry: \"%s\"\n        language: system\n        priority: 0\n' $i $i "$cmd"
      [[ $glob == "*" ]] || printf "        files: '\\\\.%s\$'\n" "${glob#*.}"
      i=$((i + 1))
    done <<<"$tasks"
  } >"$dir/.pre-commit-config.yaml"

  { # hk
    local v
    v=$(hk --version 2>/dev/null | awk '{print $2}') || v=
    printf 'amends "package://github.com/jdx/hk/releases/download/v%s/hk@%s#/Config.pkl"\n' "$v" "$v"
    printf 'hooks {\n  ["pre-commit"] {\n    steps {\n'
    i=0
    while IFS=$'\t' read -r glob cmd; do
      printf '      ["t%d"] {\n        glob = "%s"\n        check = "%s {{ files }}"\n      }\n' $i "$glob" "$cmd"
      i=$((i + 1))
    done <<<"$tasks"
    printf '    }\n  }\n}\n'
  } >"$dir/hk.pkl"

  { # overcommit: disable every built-in hook, add ad-hoc ones
    printf 'verify_signatures: false\nquiet: true\nPreCommit:\n  ALL:\n    enabled: false\n'
    i=0
    while IFS=$'\t' read -r glob cmd; do
      [[ $cmd == "$SLEEP" ]] && cmd='["sh", "-c", "sleep 0.5", "sh"]' || cmd=$(jq -Rc 'split(" ")' <<<"$cmd")
      printf '  T%d:\n    enabled: true\n    quiet: true\n    include: "**/%s"\n    command: %s\n' $i "$glob" "$cmd"
      i=$((i + 1))
    done <<<"$tasks"
  } >"$dir/.overcommit.yml"
}

# --- fixtures ----------------------------------------------------------------

# fixture <scenario> <n> -> prints pristine dir path; builds it once.
# The marker is written last: macOS purges old files under $TMPDIR but keeps
# the directories, so a directory alone doesn't mean the fixture is intact.
fixture() {
  local scenario=$1 n=$2 dir="$WORK/$1-$2/pristine" i
  if [[ -f $dir/.git/bench-ready ]]; then echo "$dir"; return; fi
  rm -rf "$dir"
  mkdir -p "$dir"
  (
    cd "$dir"
    git init -q -b main
    git config user.email bench@example.com
    git config user.name bench
    git config commit.gpgsign false
    git config core.hooksPath .git/hooks
    for ((i = 0; i < n; i++)); do echo "base $i" >"f$i.${EXTS[i % 4]}"; done
    case $scenario in
      concurrency) for e in "${EXTS[@]}"; do printf '*.%s\t%s\n' "$e" "$SLEEP"; done ;;
      partial-probe) printf '*\t%s\n' "$PROBE_CMD" ;;
      *) printf '*\t%s\n' "$NOOP" ;;
    esac | write_configs "$dir"
    git add -A && git commit -qm base --no-verify
    for ((i = 0; i < n; i++)); do echo "staged $i" >>"f$i.${EXTS[i % 4]}"; done
    git add -A
    if [[ $scenario == partial-* ]]; then
      for ((i = 0; i < n; i++)); do echo UNSTAGED_MARKER >>"f$i.${EXTS[i % 4]}"; done
    fi
    if command -v overcommit >/dev/null; then overcommit --install >/dev/null 2>&1 || true; fi
    touch .git/bench-ready
  ) >&2
  echo "$dir"
}

# --- run ---------------------------------------------------------------------

IFS=, read -ra tools <<<"$TOOLS"
IFS=, read -ra scenarios <<<"$SCENARIOS"
IFS=, read -ra counts <<<"$FILES"

available=()
for t in "${tools[@]}"; do
  if [[ $t == run-staged && -z $RUN_STAGED_BIN ]]; then
    echo "building run-staged (release)..." >&2
    cargo build --release --quiet -p run-staged --manifest-path "$ROOT/Cargo.toml"
  fi
  if tool_cmd "$t" >/dev/null 2>&1; then available+=("$t"); else echo "skipped: $t not found" >&2; fi
done
[[ ${#available[@]} -gt 0 ]] || { echo "error: no tools available" >&2; exit 1; }

rm -rf "$RESULTS" && mkdir -p "$RESULTS"
for t in "${available[@]}"; do printf '%s\t%s\n' "$t" "$(tool_version "$t")"; done >"$RESULTS/versions.tsv"

# bench <scenario> <n> <label>: one hyperfine run over all available tools.
# run_once <pristine> <work> <tool>: reset the fixture and run the tool once, untimed, stdin closed.
run_once() {
  rsync -a --delete "$1/" "$2/"
  (cd "$2" && eval "$(tool_cmd "$3")" </dev/null 2>&1)
}

# bench <scenario> <n> <label>: one hyperfine run over all tools that pass an untimed first run.
# Tools that fail are left out and listed in results/failures.tsv.
bench() {
  local scenario=$1 n=$2 label=$3 pristine work t cmd out ok=() args=()
  pristine=$(fixture "$scenario" "$n")
  work="$WORK/$scenario-$n/work"
  mkdir -p "$work"
  for t in "${available[@]}"; do
    if ! out=$(run_once "$pristine" "$work" "$t"); then
      echo "failed: $t on $label, excluded" >&2
      printf '%s\t%s\t%s\n' "$label" "$t" "$(tail -1 <<<"$out")" >>"$RESULTS/failures.tsv"
      continue
    fi
    ok+=("$t")
  done
  [[ ${#ok[@]} -gt 0 ]] || return 0
  for t in "${ok[@]}"; do
    cmd=$(tool_cmd "$t")
    # hyperfine needs at least one positional command next to --reference.
    if [[ $t == run-staged && ${#ok[@]} -gt 1 ]]; then args+=(--reference "$cmd" --reference-name "$t")
    else args+=(-n "$t" "$cmd"); fi
  done
  echo "== $label ($n files)" >&2
  (
    cd "$work"
    hyperfine -N --warmup "$WARMUP" --runs "$RUNS" --sort mean-time \
      --prepare "rsync -a --delete $pristine/ $work/" \
      --export-json "$RESULTS/$label.json" --export-markdown "$RESULTS/$label.md" \
      "${args[@]}"
  )
}

# check_unstaged <n>: run each tool once, untimed, with a probe command. Records whether the tool
# hid unstaged edits from its commands and whether the edits survived the run.
check_unstaged() {
  local n=$1 pristine work t hid kept
  mkdir -p "$WORK"
  printf '#!/bin/sh\necho >>%s/probe.ran\ngrep -l UNSTAGED_MARKER "$@" >>%s/probe.txt\nexit 0\n' "$WORK" "$WORK" >"$PROBE"
  chmod +x "$PROBE"
  pristine=$(fixture partial-probe "$n")
  work="$WORK/partial-probe-$n/work"
  mkdir -p "$work"
  for t in "${available[@]}"; do
    : >"$WORK/probe.txt"
    : >"$WORK/probe.ran"
    run_once "$pristine" "$work" "$t" >/dev/null || true
    if [[ ! -s $WORK/probe.ran ]]; then hid="probe not run"
    elif [[ -s $WORK/probe.txt ]]; then hid=no
    else hid=yes; fi
    [[ $(grep -l UNSTAGED_MARKER "$work"/f* | wc -l) -eq $n ]] && kept=yes || kept=no
    printf '%s\t%s\t%s\n' "$t" "$hid" "$kept"
  done >"$RESULTS/unstaged.tsv"
}

# verify <scenario>: run each tool once, untimed, and show failures.
verify() {
  local pristine work t out rc failed=0
  pristine=$(fixture "$1" 8)
  work="$WORK/$1-8/work"
  mkdir -p "$work"
  for t in "${available[@]}"; do
    rc=0
    out=$(run_once "$pristine" "$work" "$t") || rc=$?
    if [[ $rc -eq 0 ]]; then echo "ok   $1 $t"; else
      echo "FAIL $1 $t (exit $rc)"; tail -5 <<<"$out" | sed 's/^/     /'; failed=1
    fi
  done
  return $failed
}

if [[ $VERIFY -eq 1 ]]; then
  rc=0
  # scaling reuses the overhead fixture
  for s in $(printf '%s\n' "${scenarios[@]/scaling/overhead}" | sort -u); do verify "$s" || rc=1; done
  exit $rc
fi

for s in "${scenarios[@]}"; do
  case $s in
    overhead | concurrency) bench "$s" "$DEFAULT_N" "$s" ;;
    scaling) for n in "${counts[@]}"; do bench overhead "$n" "scaling-$n"; done ;;
    partial-staging) bench "$s" "$DEFAULT_N" "$s" && check_unstaged "$DEFAULT_N" ;;
    *) echo "skipped: unknown scenario $s" >&2 ;;
  esac
done

"$BENCH_DIR/report.sh" "$RESULTS" >"$BENCH_DIR/RESULTS.md"
echo "wrote $BENCH_DIR/RESULTS.md" >&2
