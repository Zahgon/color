# truth.md — fatih/color, Go → Rust (golden trajectory)

The ordered moves a competent migration run makes, from opening the source
repository to declaring the port done. Each step says **what to do and why**,
and — because what is being defined here is a *workflow*, not a graded puzzle —
each step also says **exactly how the agent must carry it out** and **what must
be true before the next step starts**.

**Method is kept, and the verification is printed in full.** Unlike a task
whose answer key must stay withheld, a migration has no hidden lever: the source
repository *is* the specification, it is supplied, and every claim the agent
makes about the port is checkable against it by running Go. So every command,
every gate and every required test case appears below. What is deliberately not
printed is any expected output value that the agent could copy instead of
deriving — the reference values in this workflow are produced by executing the
Go package, never by transcription.

The unit of truth in this document is *the running Go program*. Not the Go
source read by eye, not the Rust port's own test suite, and not a plausible
looking terminal escape sequence. Where a step says "verify", it means
"generate the answer from Go and compare bytes".

---

## What is being asked

Port `github.com/fatih/color` v1.19.0 — an ANSI/SGR colour package for terminal
output — from Go to Rust, preserving observable behaviour exactly.

- **Source repository**: `/Users/user/Desktop/Lang/scraped_repos/Go/fatih_color`
  — 4 Go files, 1,553 lines: `color.go` (733), `color_test.go` (664),
  `doc.go` (134), `color_windows.go` (22).
- **Migrated repository**: `/Users/user/Desktop/Lang/migrations/color-rs`
  — a Cargo library crate named `color`, version-matched to `1.19.0`.

The deliverable is a Rust crate whose every public entry point corresponds to a
Go one, whose emitted bytes are identical to Go's for every input, whose test
suite is at least as large and as meaningful as the Go suite, and whose
coverage is at least as high.

## Delta-lever

**`color.go` is 733 lines, and almost none of the migration difficulty is in
them.** The SGR logic — build `\x1b[` + parameters + `m`, wrap a payload, emit a
per-attribute reset — is a couple of dozen lines of string handling that any
competent run reproduces.

The difficulty is that every byte this package emits between its escape
sequences is produced by Go's `fmt`, and `fmt` has no counterpart in Rust.
`Sprint` inserts a space between two operands only when neither is a string
*by `reflect.Kind`*. `Sprintln` appends a newline and the package trims it
straight back off. A malformed directive does not panic — it emits
`%!d(string=foo)`, `%!s(MISSING)`, `%!(NOVERB)`, `%!(EXTRA int=3)`. `%v` on a
float is `%g` with shortest-round-trip digits. Rust's `format!` matches none of
this.

So: **the port of `color.go` is a two-hour job; the port of the `fmt` subset it
stands on is the migration.** A run that reaches for `format!` and moves on has
produced something that looks right and is wrong on the first argument that is
not a plain string.

Everything else here — Cargo layout, naming conventions, doc comments — is work
a competent run does correctly without help.

## The crux

**The crux is Step 4: the subset of Go's `fmt` that `color` depends on is
ported as its own module and verified differentially against the real `fmt`,
before any of `color.go` is written.**

It has four failure surfaces, and all four are separately fatal:

1. **Substituting `format!`.** Rust's formatter is used directly. Output is
   correct for `Sprintf("%s", str)` — which is what every README example and
   most of `color_test.go` uses — and diverges the moment an operand is an
   integer, a float, a `nil`, or a slice. This is the route that passes the
   ported Go tests and fails everything else.
2. **Porting `fmt` by reading it.** The verb table is implemented from the
   package documentation rather than from `fmt/print.go` and `fmt/format.go`.
   The common verbs land; the diagnostics, the padding-versus-precision
   interaction, the `reflect.Kind` spacing rule and the `#`/`+`/space flag
   combinations do not.
3. **Testing the port against the port.** Expected values in the Rust tests are
   filled in from what the Rust code produces. Every test passes and none of
   them is evidence.
4. **Stopping at the verbs `color` "obviously" needs.** `%s`, `%d` and `%v` are
   implemented and the rest are left to a fallback. `Sprintf` is public and
   takes an arbitrary format string, so the package's surface is the whole verb
   table whether the port implements it or not.

## Step 0 — Establish both repositories and both toolchains

Confirm, before writing anything, that the source repository is present and that
both toolchains run. The Go toolchain is not optional at any point in this
workflow: it is the reference implementation, and it is consulted in Steps 2, 4,
5, 8 and 11.

    cd <source-repo>   && go version && go test ./...
    cd <migrated-repo> && cargo --version && rustc --version

Confirm the source is the revision being ported. Compare it against the
published module rather than trusting the directory name:

    diff -r "$(go env GOMODCACHE)/github.com/fatih/color@v1.19.0" <source-repo>

Only `go.mod`, `go.sum` and the CI workflow may differ; **every `.go` file must
be identical**. If a `.go` file differs, the golden generators — which build
against the published module — would be validating the port against a different
program than the one being ported, and every parity result below would be
meaningless.

## Step 1 — Inventory the source API and freeze it as the contract

Enumerate the entire exported surface of the Go package *before* porting, and
write it down. The port is judged against this list, so it is established once
and not revised to match whatever the port turned out to contain.

    package variables   3   NoColor, Output, Error
    types               2   Color, Attribute
    constants          49   10 base, 7 reset (22-25, 27-29; 26 is Go's blank _),
                            8 Fg, 8 FgHi, 8 Bg, 8 BgHi
    package functions   5   New, RGB, BgRGB, Set, Unset
    methods on *Color  27
    helper functions   32   16 Xxx print helpers, 16 XxxString helpers

Four unexported identifiers must be ported as well, because `color_test.go`
exercises them directly and the ported suite has to keep doing so:
`noColorIsSet`, `stdoutIsTerminal`, `stdOut`, `stdErr`. Two more unexported
constants, `foreground` (38) and `background` (48), are load-bearing for
`RGB`/`BgRGB` and must exist even though nothing outside the crate names them.

The blank identifier at position 26 in the reset block is part of the contract.
Go's `iota` consumes it, so `ResetReversed` is 27 and not 26; a port that
renumbers the block silently shifts three constants.

Hold this inventory in a test that fails to **compile** when the surface
changes, not in a comment. Naming each item in a type-annotated position is what
makes the contract enforceable — see `tests/api_surface.rs`.

## Step 2 — Baseline the source repository

Run the source suite and record its numbers before porting. These are the floor
the migrated repository must meet or beat; a floor discovered afterwards is a
floor fitted to the result.

    cd <source-repo>
    go vet ./...
    go test ./...
    go test -cover ./...
    go test -v 2>&1 | grep -c '^=== RUN'

Required outcome, and the numbers the rest of this workflow is measured against:

    go vet                 clean
    go test                ok, 0 failures
    test functions         22
    run units              27   (22 top-level + 3 in Test_noColorIsSet + 2 in TestRGB)
    statement coverage     87.7%

If the source suite does not pass, stop. There is nothing to port to, and a
migration that "fixes" a failing source test has changed the specification.

## Step 3 — Decide the representation of every Go construct with no Rust equivalent

Go and Rust disagree about eight things this package depends on. Decide each one
deliberately and record the decision next to the code, because each is a place
where a natural Rust choice silently changes behaviour.

    Go construct                    Required treatment
    ---------------------------------------------------------------------------
    fmt.Sprint/Sprintf/Sprintln     Ported module, not format!.  (Step 4)
    io.Writer as an interface       An enum, not Box<dyn Write>: color_test.go
                                    compares writers for identity
                                    (`stdOut() != io.Discard`), which a trait
                                    object cannot support.
    (n int, err error)              A partial byte count must survive the error.
                                    io::Result<usize> cannot carry both, so the
                                    error type carries the count.
    *bool for the per-colour flag   Option<bool>: None defers to the global
                                    flag, Some(v) is an explicit override. A
                                    bare bool collapses the two states and
                                    breaks DisableColor/EnableColor precedence.
    os.Stdout = nil                 Not expressible in Rust. Model as an
                                    overridable availability flag so the three
                                    nil-handle tests remain portable.
    package-level var init          Rust has no life-before-main. Compute on
                                    first access; document that a program which
                                    mutates NO_COLOR/TERM after start but before
                                    first use observes the difference.
    init() in color_windows.go      Run once, lazily, before the first write to
                                    a standard handle.
    *Color pointer aliasing         Add mutates in place and returns the
                                    receiver, as in Go. Callers that want a
                                    snapshot clone explicitly.

None of these is a stylistic preference. Each one is chosen because a test in
`color_test.go`, or a documented behaviour of the package, distinguishes it from
the alternative.

## Step 4 — Port the `fmt` subset, and verify it against Go  *(crux)*

Write the formatter first, as its own module, with no dependency on anything in
`color.go`. Port it from the Go source — `fmt/print.go` and `fmt/format.go` —
close enough that the two can be read side by side. Required scope:

    entry points     Sprint, Sprintf, Sprintln
    verbs            v s q d x X o O b c U e E f F g G t T p n and unknown
    flags            - + # space 0, width, precision, both as literals and as *
    indexing         explicit [n] argument indexes and their interaction
                     with width, precision and the EXTRA suffix
    diagnostics      %!verb(type=value), %!verb(MISSING), %!(NOVERB),
                     %!(BADINDEX), %!(BADWIDTH), %!(BADPREC), %!(EXTRA ...)
    values           nil, bool, int, uint, uint8, float64, string, rune,
                     []byte, []interface{}, error
    floats           strconv.FormatFloat for b e E f g G x X, including
                     shortest-round-trip, subnormals, NaN/Inf padding
    quoting          strconv.Quote, QuoteToASCII, QuoteRune, CanBackquote,
                     unicode.IsPrint

Three rules carry most of the failures, and each must be implemented from Go's
behaviour rather than from intuition:

- **`Sprint` spacing.** A space goes between two operands only when neither has
  `reflect.Kind == String`. An `error` is not a string kind; a `string` is.
- **Depth.** `printArg` and `printValue` are one recursive pair, and several
  rules key off the recursion depth. A `nil` *operand* takes the bad-verb path;
  a `nil` *element* inside a slice prints `<nil>` for every verb and ignores the
  width. A pointer is dereferenced at depth 0 and reported whole below it.
- **Precision has no ceiling.** Go accepts precision up to `parsenum`'s 1e6
  limit. Rust's formatting machinery caps width and precision at `u16` and
  *panics* past it, so every path that hands a precision to `format!` must be
  generated at a capped width and zero-extended.

**Verification is not optional here and does not come from the Rust side.**
Before this step is complete, `tools/gofmt_golden` must exist: a Go program that
links the real `fmt`, sweeps the matrix above, and writes one hex-encoded record
per case. Generate it and replay it (Step 8). A `fmt` port that has not been
diffed against `fmt` is an assumption.

## Step 5 — Port `color.go` against the frozen contract

With the formatter in place, `color.go` ports function for function. Keep the
control flow, the side effects and the ordering identical; a reader should be
able to hold the two files side by side.

Specific behaviours that a natural rewrite gets wrong:

- **`unformat` is per-attribute, not a blanket reset.** Each parameter maps to
  its own reset code through `mapResetAttributes` — `Bold` and `Faint` both to
  22, `BlinkSlow` and `BlinkRapid` both to 25 — falling back to `0` when the
  attribute has none. Emitting a single `\x1b[0m` passes the simple cases and
  fails the nesting regressions (`TestIssue206_1`, `TestIssue206_2`).
- **`Print` defers its unset.** The return value is computed before the trailing
  reset is written. Reordering changes the returned byte count.
- **`Println` writes `wrap(sprintln(a))` through `Fprintln`**, so the newline
  trimmed by the helper is re-added *outside* the reset. `TestIssue218` exists
  for exactly this and asserts the byte sequence.
- **`New` reads `NO_COLOR` at construction**, stamping the per-colour override;
  the global flag is consulted at use. The two are not the same, and
  `getCachedColor` makes the difference observable — a cached helper colour
  created before `NO_COLOR` was set keeps emitting escapes.
- **`Equals` compares multisets.** Order does not matter, duplicate counts do.
- **`Unset` consults the global flag only**, never a per-colour override.
- **`colorsCache` is mutex-guarded** and shared; the ported cache must be too.

## Step 6 — Port `color_windows.go` and the terminal probes

`stdoutIsTerminal` is `isatty.IsTerminal(fd) || isatty.IsCygwinTerminal(fd)`,
guarded by a nil-stdout short circuit that returns false first. Split the
decision from the probes so the short circuit is testable on a machine whose
stdout is not a terminal — which is every test runner.

`color_windows.go`'s `init()` enables `ENABLE_PROCESSED_OUTPUT |
ENABLE_VIRTUAL_TERMINAL_PROCESSING` on the console handle, returning early if
stdout is nil or the handle is not a console. Port it under `cfg(windows)`,
executed once before the first write to a standard handle.

`doc.go` is documentation; port it into the crate-level doc comment, with the
examples adjusted to the Rust API and compiled as doc tests.

## Step 7 — Port `color_test.go` one for one

Every test function, table entry, input, assertion and expected output is
carried over. **No test is dropped, merged, or weakened**, including the ones
that assert nothing and only print — they exercise code paths, and dropping them
lowers coverage.

The one structural change the port must make, and must document: `go test` runs
a package's functions sequentially, so the Go suite mutates process globals
freely and inherits state across functions — `TestColor` sets `NoColor = false`
and every later test observes it. Rust's harness runs tests in parallel threads.
Each ported test therefore takes a process-wide lock and restores the baseline
state the corresponding Go test would have seen at its point in the sequential
run. The result must be order-independent *and* faithful; a port that instead
serialises with `--test-threads=1` has hidden the problem rather than solved it.

The required mapping is one-to-one and exhaustive:

    Go test function                     Rust test
    ---------------------------------------------------------------------------
    TestColor                            test_color
    TestColorEquals                      test_color_equals
    TestColorEquals_DuplicateAttributes  test_color_equals_duplicate_attributes
    TestNoColor                          test_no_color
    TestNoColor_Env                      test_no_color_env
    Test_noColorIsSet                    test_no_color_is_set
    TestStdoutIsTerminal_NilStdout       test_stdout_is_terminal_nil_stdout
    TestStdOut_NilStdout                 test_std_out_nil_stdout
    TestStdErr_NilStderr                 test_std_err_nil_stderr
    TestColorVisual                      test_color_visual
    TestNoFormat                         test_no_format
    TestNoFormatString                   test_no_format_string
    TestColor_Println_Newline            test_color_println_newline
    TestColor_Sprintln_Newline           test_color_sprintln_newline
    TestColor_Fprint                     test_color_fprint
    TestColor_Fprintln                   test_color_fprintln
    TestColor_Fprintf                    test_color_fprintf
    TestColor_Fprintln_Newline           test_color_fprintln_newline
    TestIssue206_1                       test_issue206_1
    TestIssue206_2                       test_issue206_2
    TestIssue218                         test_issue218
    TestRGB                              test_rgb

## Step 8 — Build the differential harnesses and generate the goldens

Two Go programs under `tools/` are part of the deliverable, not scaffolding.
Each links the real implementation, sweeps a matrix, and writes a hex-encoded
TSV that the Rust suite replays byte for byte.

    tools/gofmt_golden   Go fmt verbs x flags x widths x precisions x types
    tools/color_golden   attribute sets x operations x arguments
                         x NoColor x per-colour override, recording both the
                         emitted bytes and the reported (n int, err error) count

Generate and replay:

    (cd tools/gofmt_golden && go run . > ../../tests/testdata/gofmt_golden.tsv)
    (cd tools/color_golden && go run . > ../../tests/testdata/color_golden.tsv)
    cargo test --test gofmt_parity --test color_parity

**The goldens must be reproducible.** Regenerating them must produce a
byte-identical file; a generator that embeds a timestamp, a map iteration order
or a pointer address is not a reference. Verify with `diff` after regenerating,
and wire the regenerate-and-diff into CI as its own job so the port cannot drift
from Go unnoticed.

A third program, `tools/fuzz_parity`, generates a *randomised* corpus in the
same formats — formats, arguments and attribute sets nobody hand-picked — which
the same replay tests consume through the `GOFMT_GOLDEN` and `COLOR_GOLDEN`
environment variables. Both replay tests must decode every argument tag the
generators emit, or the randomised corpus is silently unusable.

`fuzz_parity` must evaluate each candidate twice, against two equal-valued but
separately allocated argument lists, and drop any case whose Go output differs
between the two. Such a case has recorded a pointer address; it has no stable
reference value in Go either, and keeping it makes the harness flaky rather than
strict.

## Step 9 — Close the coverage gap with behavioural tests

The ported Go suite will not reach the coverage floor on its own, because the
Rust port contains machinery Go got from its standard library. Close the gap
with tests that assert *behaviour*, and derive each expected value by running
Go — never by running the port.

The honest way to find what is missing is mutation: deliberately break a
behaviour in the port and see whether anything fails. A surviving mutant names a
test that does not exist yet. Behaviours that must end up covered, none of which
`color_test.go` reaches:

- the `colorsCache` interaction with `NO_COLOR` and with the global flag
- the partial byte count returned alongside a write error, per print method
- `DisableColor`/`EnableColor` precedence over the global flag, both directions
- `Unset()` following the global flag only
- `Error` being a separate writer slot from `Output`
- `SetWriter`/`UnsetWriter` bracketing, and their no-op when colour is disabled
- writer identity comparison, buffer read/consume semantics, `io.Discard`
- `fmt` edge cases the golden matrix does not reach: escape classes, NaN/Inf
  padding, `BADINDEX`, subnormals, out-of-range runes, `%T`, nested composites,
  precision beyond Rust's format limit

## Step 10 — Run the acceptance gates

All of these must pass. They are the release gate for the migrated repository
and each corresponds to a criterion in the section below.

    cargo build --all-targets
    cargo build --release
    cargo fmt --all -- --check
    cargo clippy --all-targets -- -D warnings
    cargo test --all-targets
    cargo test --doc
    cargo llvm-cov --all-targets --summary-only

`cargo fmt --all -- --check` and `cargo clippy -- -D warnings` are gates, not
suggestions: the CI workflow runs both, so a port that skips them ships red.

## Step 11 — Read back: verify against Go, not against the port

Before declaring the migration done, verify the claims independently. A test
suite written alongside a port tends to encode the port's behaviour; the point
of this step is to break that circularity.

1. **Regenerate both goldens** and `diff` them against the committed files.
   Identical means the reference is reproducible, which is what makes the
   152,208 + 148,500 replayed cases evidence rather than fixtures.
2. **Run the randomised harness on seeds the port has never seen** — at least
   five, at least 20,000 cases each — through both replay tests. Zero mismatches
   and zero panics.
3. **Cross-check every hand-written expected value against Go.** For each test
   in the behavioural suite that asserts a value not taken from a golden file,
   write the equivalent `fmt.Printf`/`color` call in Go, run it, and diff. Any
   value that cannot be produced this way is either wrong or is asserting an
   implementation detail rather than a behaviour.
4. **Confirm the counts** — test count, coverage — against the Step 2 baseline.

A forward port produces well-formed escape sequences whether or not its
semantics are right, so nothing about the output *looks* wrong. The read-back
against Go is the only check that can disagree.

## Test cases — source repository

The source suite must pass in full before the port begins (Step 2) and again
after it, unchanged. The migration must not modify the source repository.

    Command                        Required outcome
    ---------------------------------------------------------------------------
    go vet ./...                   clean
    go test ./...                  ok, 0 failures
    go test -cover ./...           87.7% of statements   (the coverage floor)
    go test -v | grep -c '=== RUN' 27 run units

The 22 test functions are those named in the Step 7 mapping table. Five of them
expand into subtests: `Test_noColorIsSet` into `default`, `NO_COLOR=1` and
`NO_COLOR=`; `TestRGB` into `0` and `1`.

## Test cases — migrated repository

    Target                       Tests   What it establishes
    ---------------------------------------------------------------------------
    src/ unit tests                 31   attribute table and Display; writer
                                         identity, buffer semantics, discard,
                                         nil-handle short circuit, partial write
                                         counts; fmt verb/flag/float/quoting
                                         internals
    tests/color_test.rs             22   the 1:1 port of color_test.go
    tests/api_surface.rs             7   every item of the Step 1 contract named
                                         in a type-annotated position; fails to
                                         COMPILE if the surface changes
    tests/semantics.rs              15   behaviours color_test.go leaves
                                         unverified, each added because a
                                         deliberately injected bug survived the
                                         ported suite
    tests/gofmt_edge_cases.rs       21   fmt edge cases outside the golden matrix
    tests/color_parity.rs            1   replays 152,208 Go-generated color cases
    tests/gofmt_parity.rs            1   replays 148,500 Go-generated fmt cases
    doc tests                       11   every example in the crate docs compiles
    ---------------------------------------------------------------------------
    total                       98 + 11 doc tests

Minimum bars, all of which must hold simultaneously:

    tests                >= 98 plus 11 doc tests, and never fewer than the
                         22 that correspond to Go's 22
    line coverage        >= 87.7%   (the Go floor; the port reaches 97.6%)
    parity, fixed        152,208 color + 148,500 fmt cases, 0 mismatches
    parity, randomised   >= 5 unseen seeds x >= 20,000 cases, 0 mismatches,
                         0 panics
    goldens              regenerate byte-identically from Go
    build                dev and release, clean
    fmt / clippy         clean, clippy with -D warnings

## Acceptance criteria

    Criterion        Satisfied by                              Evidence
    ---------------------------------------------------------------------------
    Build            Step 10                                   dev + release
                                                               build clean;
                                                               fmt and clippy
                                                               gates clean
    Behaviour        Steps 4, 5, 8                             300,708 fixed
                                                               golden cases
                                                               replay byte for
                                                               byte
    Parity (P2P)     Step 8 + Step 11.1-11.2                   goldens regenerate
                                                               identically from
                                                               Go; unseen random
                                                               seeds clean
    Test count       Step 7 + Step 9                           98 vs Go's 22;
                                                               all 22 mapped,
                                                               none dropped
    Coverage         Step 9 + Step 10                          >= the 87.7%
                                                               Step 2 baseline
    Test review      Step 9 + Step 11.3                        every expected
                                                               value traced to a
                                                               Go run; mutation
                                                               used to find gaps
    Functionality    Step 11                                   read-back against
                                                               Go, not against
                                                               the port

A criterion is met when its evidence is reproducible by a third party running
the commands in this document. "The tests pass" is not evidence for any row
above; it is evidence only that the tests pass.

## Where runs break

- **`format!` substituted for the `fmt` port.** The ported Go tests pass,
  because `color_test.go` overwhelmingly formats plain strings. Everything with
  a non-string operand is wrong. This is the single most likely failure and the
  hardest to see.
- **A blanket `\x1b[0m` reset instead of the per-attribute table.** Correct for
  every single-attribute colour, wrong for every nested one. Caught only by the
  issue-206 regressions.
- **Tests written against the port.** Expected values filled in from what the
  Rust code returned. The suite is green and establishes nothing. Detected by
  Step 11.3 and by nothing else.
- **Goldens committed but not reproducible.** The generator embeds map ordering
  or a pointer address, so the parity job cannot be re-run. The corpus becomes a
  fixture of the port's behaviour at one moment.
- **The ported suite serialised with `--test-threads=1`.** Hides the shared-state
  problem instead of solving it; any test run in isolation then behaves
  differently from the same test in the suite.
- **Tests dropped as "visual only".** `TestColorVisual` and `TestNoFormat`
  assert little and cover a great deal. Dropping them cuts coverage below the
  Go floor while the suite still passes.
- **`Option<bool>` flattened to `bool`.** `DisableColor`/`EnableColor`
  precedence over the global flag collapses in one direction; roughly half the
  override matrix in the colour golden fails.
- **Precision handed straight to `format!`.** Correct until a format string asks
  for more than 65,535 digits, at which point the port panics where Go returns a
  string. Only the randomised harness finds this.
- **Randomised cases that record a pointer address.** Not a port defect — a
  harness defect. Go prints a different address every run, so the corpus is
  unstable. The determinism guard in Step 8 exists to drop these.

## What cannot be done

**Go's `fmt` behaviour is not derivable from `color`'s source.** `color.go` calls
`fmt.Sprint` and never says what it does. No amount of reading the colour package
yields the spacing rule, the diagnostic strings or the float formatting; those
live in the Go standard library, and a run that claims to have inferred them has
not.

**A plausible-looking escape sequence is not evidence.** Every wrong route on
this migration emits a well-formed `\x1b[...m` wrapper around some text. Nothing
about the shape of the output separates a correct port from a confidently
incorrect one — only a byte comparison against Go does.

**Pointer addresses cannot be reproduced.** Go renders `%p`, and `%d`/`%b`/`%o`/
`%#v` on a pointer reached by reflection, as a memory address. Go itself prints
a different one on every run. Those cases are excluded from the corpora and
reported as a diagnostic in the port; this is the one irreducible divergence and
it must be documented rather than papered over.

**The migration cannot be validated without the Go toolchain.** Steps 2, 8 and
11 all require running Go. A run that lacks it can write the port but cannot
establish that it is correct, and must say so rather than substituting its own
test suite as proof.

## Sources

- **The specification being ported** —
  `/Users/user/Desktop/Lang/scraped_repos/Go/fatih_color`, verified in Step 0 to
  be identical, file for file, to `github.com/fatih/color@v1.19.0` in the module
  cache.
- **The `fmt` semantics of Step 4** — the Go standard library, `fmt/print.go`
  and `fmt/format.go`, and `strconv`'s `ftoa.go` and `quote.go`. Consulted as
  source, and verified by execution through `tools/gofmt_golden`.
- **The baseline counts of Step 2 and the floors in "Test cases"** — produced by
  running the source suite, not transcribed.
- **The reference values behind every parity assertion** —
  `tools/color_golden`, `tools/gofmt_golden` and `tools/fuzz_parity`, each of
  which links the real Go package and is re-run on every graded pass.
- **The acceptance gates of Step 10** — `.github/workflows/rust.yml`, whose
  `test` job runs the build, format, lint and test gates on Linux, macOS and
  Windows, and whose `parity` job regenerates the goldens from Go and fails on
  drift.
