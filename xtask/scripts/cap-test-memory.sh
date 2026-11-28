#!/bin/sh
# Runs one mutation test process and kills it, with every process it started, when any of them uses
# more than the memory cap that `cargo xtask mutation` computes. The cap is per process, as a
# resource limit would be, because a test that runs the package tooling needs more memory than a
# runner can give to one process.
if [ -z "$MUTATION_TEST_MEMORY_BYTES" ]; then
  echo "cap-test-memory: MUTATION_TEST_MEMORY_BYTES is not set; cargo xtask mutation sets it." >&2
  exit 1
fi

case "$MUTATION_TEST_MEMORY_BYTES" in
  *[!0-9]*)
    echo "cap-test-memory: MUTATION_TEST_MEMORY_BYTES must be a positive byte count, not '$MUTATION_TEST_MEMORY_BYTES'." >&2
    exit 1
    ;;
esac

if [ "$MUTATION_TEST_MEMORY_BYTES" -eq 0 ]; then
  echo "cap-test-memory: MUTATION_TEST_MEMORY_BYTES must be a positive byte count, not 0." >&2
  exit 1
fi

polls=0
test_process=

descendants() {
  # Prints "pid resident_kib" for each of the given roots and every process below them. A root must
  # still be alive: a dead root has no children left to find, because the kernel moves them to
  # init.
  ps -e -o pid=,ppid=,rss= | awk -v roots="$*" '
    BEGIN { count = split(roots, list, " "); for (slot = 1; slot <= count; slot++) root[list[slot]] = 1 }
    { parent[$1] = $2; usage[$1] = $3 }
    END {
      for (process in parent) {
        ancestor = process
        for (depth = 0; depth < 20 && ancestor > 1; depth++) {
          if (ancestor in root) {
            print process, usage[process]
            break
          }
          ancestor = parent[ancestor]
        }
      }
    }'
}

stop_test() {
  # $1 is the signal and $2 the victims found before the first signal, so the second signal still
  # reaches a process whose parent died and was reparented. The test process goes first, so it
  # starts nothing else while its children are killed.
  kill -"$1" "$test_process" 2>/dev/null

  for victim in $2; do
    kill -"$1" "$victim" 2>/dev/null
  done
}

escalate() {
  stop_tree

  exit 143
}

stop_tree() {
  # The test process goes first, so it starts nothing else while its children are killed. The
  # victims are found once, so the second signal still reaches a process whose parent died and was
  # reparented.
  victims=$(descendants "$test_process" | awk '{ print $1 }')

  stop_test TERM "$victims"
  sleep 1

  # A process that ignores TERM can start children during the grace period, so look again below
  # the roots that were alive when the first signal went out.
  victims="$victims $(descendants $victims | awk '{ print $1 }')"

  stop_test KILL "$victims"
}

trap 'escalate' TERM INT HUP

# A ps that cannot report resident memory would leave the cap silently off, so check it once.
if ! ps -e -o pid=,ppid=,rss= | awk 'NF == 3 { found = 1 } END { exit !found }'; then
  echo "cap-test-memory: ps cannot report pid, ppid, and resident memory, so the cap cannot work." >&2
  exit 1
fi

# A sleep that rejects fractions would turn the scan cadence into seconds without saying so.
if ! sleep 0.01 2>/dev/null; then
  echo "cap-test-memory: sleep rejects fractional seconds, so the scan cannot keep its cadence." >&2
  exit 1
fi

# The explicit stdin redirect keeps the test's stdin, which an asynchronous list in a shell without
# job control would otherwise replace with /dev/null.
"$@" <&0 &
test_process=$!

while kill -0 "$test_process" 2>/dev/null; do
  # A test that lasts less than the first scan cannot reach the cap, and the scan costs more than
  # the test does, so wait briefly before the first scan and then scan every fifth poll.
  polls=$((polls + 1))
  if [ "$polls" -lt 5 ]; then
    sleep 0.01
    continue
  fi

  sleep 0.05

  if [ $((polls % 5)) -eq 0 ]; then
    tree=$(descendants "$test_process")
    offenders=$(printf '%s\n' "$tree" | awk -v cap="$MUTATION_TEST_MEMORY_BYTES" '$2 * 1024 > cap { print $1 }')

    if [ -n "$offenders" ]; then
      echo "cap-test-memory: processes $offenders passed $MUTATION_TEST_MEMORY_BYTES bytes, so the test was killed." >&2
      stop_tree
      break
    fi
  fi
done

wait "$test_process"
