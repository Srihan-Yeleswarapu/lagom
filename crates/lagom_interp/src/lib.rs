//! The Lagom interpreter (M0) — the reference executor over MIR.
//!
//! Spec anchor: doc 08 (the interpreter is the M0 execution backend),
//! 00 §19.1 (script model), §22.2 (the semantic core), §26.5 (the LOM ring:
//! dev builds record events; the failure report names the failing value's
//! origin), docs/13 S-9/G-2 (the one formatting place), G-4 (check-failure
//! rendering), D-34 (option printing).
//!
//! Design points, each pinned by a test:
//! - **Determinism** (§26.5's replay story): `random` is drawn from an
//!   injected, seeded RNG; `ask` from injected stdin; output goes to an
//!   injected sink. Execution is a pure function of (program, inputs, seed).
//! - **The error model is the CFG, not an exception**: `Term::Fail` routes to
//!   its named pad (or leaves the function); instruction-level failures
//!   (failing `Call`, `Convert`, index errors) route to the *executing
//!   block's* `pad_on_fail` (or leave the function). Unwinding is explicit
//!   and follows the precomputed edges — no hidden control flow (13.1).
//! - **Traps are crashes, not errors** (13.1): integer overflow (the
//!   i64-checked-arithmetic rule) and division by zero abort with the
//!   panic format; they are not catchable.
//! - **LOM in dev mode**: every `Event*` instruction records into a bounded
//!   `EventRing` (the pass's budget); on unhandled failure the runtime
//!   renders `failure_report` v1 from doc 09 — "a passing dev build explains
//!   nothing extra".

use lagom_diagnostics::Span;
use lagom_sema::Type;
use lagom_mir::{
    BinOp, BlockId, Conv, EventRing, FileOp, FormatPart, Instr, LocalId, LomConfig, LomEvent,
    MathOp, MirFunction, MirItem, MirProgram, Operand, Term, TextOp, UnOp, failure_report,
    instrument,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write as _;

// ---------------------------------------------------------------------------
// Values (the M0 runtime set — S-1/S-13)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Value {
    Number(i64),
    Decimal(f64),
    Text(String),
    Boolean(bool),
    /// The option sentinel (8.5, D-34).
    Nothing,
    /// Lists and maps are runtime-managed values at M0 (ARC arrives with
    /// M1's reference types). M0 programs cannot observe binding-level
    /// aliasing: every mutation goes through a named place, and a place has
    /// one base binding.
    List(Vec<Value>),
    /// Insertion-ordered key/value pairs (S-9 prints in insertion order).
    Map(Vec<(Value, Value)>),
    Pair(Box<Value>, Box<Value>),
    /// A structure value, fields in declaration order. A *variant* value
    /// (7.12) is the same shape named by its variant.
    Struct { name: String, fields: Vec<Value> },
    /// A class instance (10.2) — an ARC'd reference (9.3): every copy of the
    /// binding shares the same `RefCell` field storage, so `set`/`increase`
    /// through any alias mutate the one object, and equality is identity
    /// (R-6: two distinct objects with equal fields are never `equal to`).
    /// The second cell is the finalizer hook (10.4): when the LAST `Rc` to
    /// this storage drops, the hook queues the object for its class's
    /// `before last reference disappears` body (run by the VM, never inside
    /// the drop itself). `None` on finalizer receivers — a cleanup body must
    /// not re-arm its own hook.
    Object {
        name: String,
        fields: std::rc::Rc<std::cell::RefCell<Vec<Value>>>,
        on_drop: Option<std::rc::Rc<DropHook>>,
    },
    /// A closure value (11.1): the synthetic function's name plus the
    /// captured values. Both backends use the same shape.
    Closure { function: String, captures: Vec<Value> },
    /// A typed channel (14.3): a FIFO queue of messages. Copies of the
    /// binding share the queue (the ARC'd shape) — send appends, receive
    /// dequeues, and the queue itself is the synchronization.
    Channel(std::rc::Rc<std::cell::RefCell<Vec<Value>>>),
}

/// 10.4: the finalizer hook shared by every handle to one object. The hook
/// holds a second `Rc` to the field storage (never to a `Value` — that would
/// cycle and leak). When the last object handle drops, the hook — still
/// holding the storage alive — fires and queues `(class, storage)` for the
/// VM: the class's `before last reference disappears` body runs at the next
/// drain point, with the fields still readable. The body itself is an
/// ordinary receiver-first function (`deinit <class>`); R-20.3's
/// cannot-fail rule is enforced by the checker, so the drain never handles
/// a failure.
#[derive(Debug)]
pub struct DropHook {
    name: String,
    fields: std::rc::Rc<std::cell::RefCell<Vec<Value>>>,
    owner: u64,
}

impl Drop for DropHook {
    fn drop(&mut self) {
        PENDING_DEINIT
            .with(|q| q.borrow_mut().push((self.owner, self.name.clone(), self.fields.clone())));
    }
}

// Objects whose last reference dropped mid-run queue here (the `Drop` impl
// runs without a VM handle, so the queue is thread-local); the VM adopts
// the queue at its next drain point and runs each object's finalizer.
thread_local! {
    static PENDING_DEINIT:
        RefCell<Vec<(u64, String, std::rc::Rc<std::cell::RefCell<Vec<Value>>>)>> =
        const { RefCell::new(Vec::new()) };
}

/// Interp instance ids (see `Interp::id`).
static NEXT_INTERP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl Value {
    /// S-9 (one formatting place, shared by `say`, interpolation, and
    /// `text from`): numbers without a decimal point, decimals
    /// shortest-roundtrip (docs/13 G-2), text raw, booleans `true`/`false`,
    /// lists `[a, b, c]`, maps `{k: v}`, options value-or-`nothing` (D-34).
    pub fn format(&self) -> String {
        match self {
            Value::Number(n) => n.to_string(),
            Value::Decimal(d) => format_decimal(*d),
            Value::Text(s) => s.clone(),
            Value::Boolean(b) => (if *b { "true" } else { "false" }).to_string(),
            Value::Nothing => "nothing".to_string(),
            Value::List(items) => {
                let inner: Vec<String> = items.iter().map(Value::format).collect();
                format!("[{}]", inner.join(", "))
            }
            Value::Map(entries) => {
                let inner: Vec<String> = entries
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k.format(), v.format()))
                    .collect();
                format!("{{{}}}", inner.join(", "))
            }
            Value::Pair(a, b) => format!("({}, {})", a.format(), b.format()),
            Value::Struct { name, fields } => {
                let inner: Vec<String> = fields.iter().map(Value::format).collect();
                format!("{}({})", name, inner.join(", "))
            }
            // S-9's class rendering: the indefinite article marks the value
            // as a reference object, not a struct — `a counter(5)`.
            Value::Object { name, fields, .. } => {
                let inner: Vec<String> = fields.borrow().iter().map(Value::format).collect();
                format!("a {}({})", name, inner.join(", "))
            }
            Value::Closure { .. } => "a function".to_string(),
            // 14.3: a channel prints as the messages it still holds — the
            // student-visible queue, never an opaque handle.
            Value::Channel(q) => {
                let inner: Vec<String> = q.borrow().iter().map(Value::format).collect();
                format!("a channel holding [{}]", inner.join(", "))
            }
        }
    }
}

/// G-2: decimals print shortest-roundtrip (Rust's `{}` for f64 already is).
/// Non-finite results render in words — no silent `NaN` in a student's face.
fn format_decimal(d: f64) -> String {
    if d.is_nan() {
        return "not a number".to_string();
    }
    if d.is_infinite() {
        return if d > 0.0 { "infinity" } else { "-infinity" }.to_string();
    }
    d.to_string()
}

// ---------------------------------------------------------------------------
// Panics — crashes, not errors (13.1): not catchable, abort the run
// ---------------------------------------------------------------------------

/// An unrecoverable runtime failure (13.1's panic class: assertion failures,
/// overflow traps, impossible conditions). Distinct from the `fail with`
/// error model, which is a *value* flowing to a pad.
#[derive(Debug, Clone, PartialEq)]
pub struct Trap {
    pub message: String,
    pub span: Span,
}

impl Trap {
    /// The teaching-panic format: what went wrong, with the line.
    fn render(&self, src: &str) -> String {
        let line = lagom_mir::line_of(src, self.span.start);
        let mut out = String::new();
        let _ = write!(out, "panicked on line {line}: {}", self.message);
        out
    }
}

/// How a function's execution ended.
enum Exit {
    /// `gives back value` — `Nothing` for fall-out of a script/test body.
    Return(Value),
    /// `fail with` / a failing instruction reached a `catch: None` boundary:
    /// the error flows to the *caller*'s innermost pad (13.1).
    Fail(String),
    /// A panic: aborts everything (not catchable).
    Trap(Trap),
}

// ---------------------------------------------------------------------------
// The host: injected IO + deterministic randomness
// ---------------------------------------------------------------------------

/// Where `say` output goes, where `ask` input comes from, and the seed for
/// `random` — all injected, so runs are testable, deterministic, and
/// replayable (§26.5's replay story: strict evaluation + fixed seed).
pub struct Host {
    /// Answer lines consumed one at a time by `ask`.
    pub stdin: Vec<String>,
    /// Everything `say` printed, in order.
    pub stdout: Vec<String>,
    /// The working directory for file operations (§19.1's `files` module).
    /// Tests point it at a temp dir; the CLI leaves it at the process cwd.
    pub workdir: std::path::PathBuf,
    rng: u64,
}

impl Host {
    pub fn new(stdin: Vec<String>) -> Host {
        Host { stdin, stdout: Vec::new(), workdir: std::env::current_dir().unwrap_or_default(), rng: 0x2545_F491_4F6C_DD1D }
    }

    /// A fixed seed, for replay-determinism tests.
    pub fn with_seed(stdin: Vec<String>, seed: u64) -> Host {
        Host { stdin, stdout: Vec::new(), workdir: std::env::current_dir().unwrap_or_default(), rng: seed | 1 }
    }

    /// A host pinned to a working directory (file-op tests, validation
    /// projects running in a sandbox dir).
    pub fn with_workdir(stdin: Vec<String>, dir: std::path::PathBuf) -> Host {
        Host { stdin, stdout: Vec::new(), workdir: dir, rng: 0x2545_F491_4F6C_DD1D }
    }



    fn next_random(&mut self) -> u64 {
        // xorshift64* — deterministic, full-period, portable.
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

// ---------------------------------------------------------------------------
// The interpreter
// ---------------------------------------------------------------------------

/// The result of running a whole program.
#[derive(Debug, Clone, PartialEq)]
pub enum RunOutcome {
    /// The script completed.
    Completed,
    /// An unhandled failure left the program: the message plus, in dev
    /// builds, the rendered LOM report (the 90/10 payoff).
    Failed { message: String, report: Option<String> },
    /// A panic (13.1): a crash, rendered with its line — plus, in dev
    /// builds, the LOM report (26.5: any unhandled failure explains itself).
    Panicked { message: String, report: Option<String> },
}

/// The result of one `test` block (S-12).
#[derive(Debug, Clone, PartialEq)]
pub struct TestOutcome {
    pub name: String,
    pub passed: bool,
    /// Why it failed: the failure message or the rendered panic.
    pub message: Option<String>,
    /// The LOM failure report for a failed test (dev builds, 26.5): what
    /// the program was doing and where each value came from.
    pub report: Option<String>,
}

pub struct Interp {
    pub host: Host,
    /// The LOM ring — present only in dev builds (release records nothing:
    /// the probes are stripped by the instrument pass).
    ring: Option<EventRing>,
    /// Whether LOM probes exist in this program (dev) — drives capture.
    dev: bool,
    /// Structure field names in declaration order — `FieldGet`/`FieldSet`
    /// resolve by name against this (MIR carries field *names*; the
    /// declaration order lives in the program table).
    struct_fields: HashMap<String, Vec<String>>,
    /// The entry frame's final named locals — `(name, type word, printed
    /// value)` in declaration order, captured when the script body returns.
    /// This is the REPL's teaching data (26.4: inferred types shown after
    /// each line); compiler temporaries (`%…`) are excluded. Values are kept
    /// in their *printed* form so the snapshot never roots a class object:
    /// a rooted object's last reference would never disappear and its
    /// finalizer would never run. Empty outside a run.
    entry_scope: Vec<(String, Type, String)>,
    /// 10.4 finalizers: class name → the `deinit <class>` function.
    deinits: HashMap<String, String>,
    /// 12.3/10.6 body-side dispatch: interface name → (class → implementing
    /// callee). A call to `iface <I> <m>` looks up the receiver's runtime
    /// class here — the vtable for "the concrete type is not known".
    iface_dispatch: HashMap<String, HashMap<String, String>>,
    /// This instance's id: drop-hook queue entries carry the id of the run
    /// that created their object, and a drain adopts only its own — entries
    /// from an already-discarded run (the REPL re-runs per submit) die with
    /// it instead of firing inside some later session.
    id: u64,
    /// 14.2 structured tasks: the tasks this function has spawned, in spawn
    /// order, as `(closure, keep_going)`. Per-run (a script or test body is
    /// one region at a time); the join consumes the queue. Tasks spawned by
    /// different functions of one run share the queue — the join at the
    /// innermost region drains what that region spawned.
    tasks: Vec<(Value, bool)>,
}

impl Interp {
    /// A dev-build interpreter: instruments the program (the named pass,
    /// 26.5) and captures events into a bounded ring.
    pub fn dev(prog: &mut MirProgram) -> Interp {
        instrument(prog, LomConfig::DEV);
        Interp::from_program(prog, true)
    }

    /// A release-build interpreter: instruments (to strip, idempotently) and
    /// records nothing — release builds are silent (the release-identity
    /// invariant, §26.5).
    pub fn release(prog: &mut MirProgram) -> Interp {
        instrument(prog, LomConfig::RELEASE);
        Interp::from_program(prog, false)
    }

    fn from_program(prog: &MirProgram, dev: bool) -> Interp {
        let struct_fields = prog
            .structs
            .iter()
            .map(|s| (s.name.clone(), s.fields.iter().map(|(n, _)| n.clone()).collect()))
            .collect();
        let deinits: HashMap<String, String> = prog.deinits.iter().cloned().collect();
        let iface_dispatch: HashMap<String, HashMap<String, String>> = prog
            .iface_dispatch
            .iter()
            .map(|(i, rows)| {
                (
                    i.clone(),
                    rows.iter().cloned().collect::<HashMap<String, String>>(),
                )
            })
            .collect();
        let ring = if dev { Some(EventRing::new(256)) } else { None };
        // Default runs vary: the seed comes from the clock (two runs of the
        // same program play differently). Replay-determinism is explicit —
        // assign a `Host::with_seed` host (26.5's fixed-seed replay).
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64 ^ d.as_secs())
            .unwrap_or(0x2545_F491_4F6C_DD1D);
        Interp {
            host: Host::with_seed(Vec::new(), nanos),
            ring,
            dev,
            struct_fields,
            entry_scope: Vec::new(),
            deinits,
            iface_dispatch,
            id: NEXT_INTERP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            tasks: Vec::new(),
        }
    }

    pub fn host(&mut self) -> &mut Host {
        &mut self.host
    }

    pub fn ring(&self) -> Option<&EventRing> {
        self.ring.as_ref()
    }

    /// The entry frame's final named locals — `(name, type, printed value)`
    /// — filled by the last `run` of a program with a script body (26.4's
    /// REPL data). Printed, not live: see the field docs.
    pub fn entry_scope(&self) -> &[(String, Type, String)] {
        &self.entry_scope
    }

    /// Run the script body (§19.1) to completion, or to the first unhandled
    /// failure/panic. `src` turns spans into line numbers for the panic
    /// format and the failure report.
    pub fn run(&mut self, prog: &MirProgram, src: &str) -> RunOutcome {
        let functions: HashMap<&str, &MirFunction> = prog
            .items
            .iter()
            .filter_map(|i| match i {
                MirItem::Function(f) => Some((f.name.as_str(), f)),
                _ => None,
            })
            .collect();

        // Only a non-empty script lowers to a `Main` item, and it is always
        // the entry — so "no Main" simply means nothing to run.
        let main = prog
            .items
            .iter()
            .find_map(|i| match i {
                MirItem::Main(f) if f.is_entry => Some(f),
                _ => None,
            });
        let Some(main) = main else {
            return RunOutcome::Completed;
        };
        let outcome = self.call_function(main, &[], &functions);
        // 10.4: the program's last drop sweep. Whether the body completed,
        // failed, or trapped, every object still held by the entry frame
        // drops as it unwinds and its finalizer runs here — deterministic
        // end-of-program cleanup, never skipped.
        self.drain_deinits(&functions);
        match outcome {
            Ok(()) | Err(Exit::Return(_)) => RunOutcome::Completed,
            Err(Exit::Fail(message)) => {
                let report = self.ring.as_ref().map(|r| failure_report(r, &main.name, src));
                RunOutcome::Failed { message, report }
            }
            Err(Exit::Trap(t)) => {
                let report = self.ring.as_ref().map(|r| failure_report(r, &main.name, src));
                RunOutcome::Panicked { message: t.render(src), report }
            }
        }
    }

    /// Run every `test` block (S-12): each test is a body executed on the
    /// interpreter; a `check that` failure or any other trap fails the test.
    /// Results come back in declaration order.
    pub fn run_tests(&mut self, prog: &MirProgram, src: &str) -> Vec<TestOutcome> {
        let functions: HashMap<&str, &MirFunction> = prog
            .items
            .iter()
            .filter_map(|i| match i {
                MirItem::Function(f) => Some((f.name.as_str(), f)),
                _ => None,
            })
            .collect();
        let tests: Vec<&MirFunction> = prog
            .items
            .iter()
            .filter_map(|i| match i {
                MirItem::Test(f) => Some(f),
                _ => None,
            })
            .collect();
        let mut out = Vec::with_capacity(tests.len());
        for t in tests {
            let outcome = self.call_function(t, &[], &functions);
            out.push(match outcome {
                Ok(()) | Err(Exit::Return(_)) => TestOutcome {
                    name: t.name.clone(),
                    passed: true,
                    message: None,
                    report: None,
                },
                Err(Exit::Fail(message)) => TestOutcome {
                    name: t.name.clone(),
                    passed: false,
                    message: Some(message),
                    report: self
                        .ring
                        .as_ref()
                        .map(|r| failure_report(r, &t.name, src)),
                },
                Err(Exit::Trap(trap)) => TestOutcome {
                    name: t.name.clone(),
                    passed: false,
                    message: Some(trap.render(src)),
                    report: self
                        .ring
                        .as_ref()
                        .map(|r| failure_report(r, &t.name, src)),
                },
            });
        }
        out
    }

    /// Call one function with argument values.
    /// 14.2: the region join — run every queued task to completion, in spawn
    /// order, then report the first failure that supervision did not consume
    /// (`keep going`). Deterministic by construction: one order, both
    /// backends. `Err` carries that failure message for the caller to route
    /// to its pad (catchable by `attempt`, unhandled otherwise).
    fn drain_tasks(&mut self, functions: &HashMap<&str, &MirFunction>) -> Result<(), String> {
        let tasks = std::mem::take(&mut self.tasks);
        let mut first_failure: Option<String> = None;
        for (task, keep_going) in tasks {
            let Value::Closure { function, captures } = task else {
                continue;
            };
            // The uniform closure ABI: the packed args list is the synthetic
            // function's one parameter (formals first — a zero-param task
            // body's list is exactly its captures, which `MakeClosure` packed
            // in formals-then-captures order... zero formals → captures only).
            let list = Value::List(captures);
            match self.call_user(&function, vec![list], functions) {
                CallResult::Done | CallResult::Value(_) => {}
                CallResult::Fail(msg) => {
                    if !keep_going && first_failure.is_none() {
                        first_failure = Some(msg);
                    }
                }
                CallResult::Exit(Exit::Trap(t)) => {
                    if !keep_going && first_failure.is_none() {
                        first_failure = Some(t.message);
                    }
                }
                CallResult::Exit(e) => {
                    // A task's own uncatchable exit (a `stop` escaping its
                    // body, say) would be a checker bug; treat as a trap.
                    if let Exit::Trap(t) = e {
                        if first_failure.is_none() {
                            first_failure = Some(t.message);
                        }
                    }
                }
            }
        }
        match first_failure {
            Some(msg) => Err(msg),
            None => Ok(()),
        }
    }

    /// 10.4: run the finalizer bodies of every object whose last reference
    /// dropped since the last drain, in drop order. Each body is the class's
    /// receiver-first `deinit <class>` function; its receiver is rebuilt with
    /// an inert hook (`on_drop: None`) so a cleanup body can never re-arm or
    /// re-fire itself. R-20.3 guarantees the bodies cannot fail, so nothing
    /// here needs failure routing — an internal error would be a checker bug.
    fn drain_deinits(&mut self, functions: &HashMap<&str, &MirFunction>) {
        let mut queued: Vec<(u64, String, std::rc::Rc<std::cell::RefCell<Vec<Value>>>)> =
            Vec::new();
        PENDING_DEINIT.with(|q| queued.append(&mut q.borrow_mut()));
        let mine: Vec<_> = queued
            .into_iter()
            .filter(|(owner, name, _)| *owner == self.id && self.deinits.contains_key(name))
            .map(|(_, name, storage)| (name, storage))
            .collect();
        for (class, storage) in mine {
            let Some(deinit_name) = self.deinits.get(&class).cloned() else {
                continue;
            };
            let Some(f) = functions.get(deinit_name.as_str()) else {
                continue;
            };
            let receiver = Value::Object {
                name: class.clone(),
                fields: storage.clone(),
                on_drop: None,
            };
            let result = self.call_function(f, &[receiver], functions);
            debug_assert!(
                matches!(result, Ok(()) | Err(Exit::Return(_))),
                "finalizer bodies cannot fail (R-20.3) or trap"
            );
        }
    }

    fn call_function(
        &mut self,
        f: &MirFunction,
        args: &[Value],
        functions: &HashMap<&str, &MirFunction>,
    ) -> Result<(), Exit> {
        // One frame: locals indexed by LocalId.
        let mut locals: Vec<Value> = vec![Value::Nothing; f.locals.len()];
        for (i, param) in f.params.iter().enumerate() {
            locals[param.0] = args.get(i).cloned().unwrap_or(Value::Nothing);
        }
        let mut pc = BlockId(0);
        loop {
            let block = f.block(pc);
            // Instruction-level failures route to this block's pad — the
            // precomputed CFG edge (22.2). `Term::Fail` carries its own
            // target; this one covers failing `Call`/`Convert`/index ops.
            let pad = block.pad_on_fail;
            let mut jumped: Option<(BlockId, Option<String>)> = None;
            for instr in &block.instrs {
                match self.exec_instr(instr, f, &mut locals, functions, pad) {
                    Step::Continue => {}
                    Step::Jump(next, msg) => {
                        jumped = Some((next, msg));
                        break;
                    }
                    Step::Exit(e) => return Err(e),
                }
            }
            if let Some((next, msg)) = jumped {
                // A failure routed mid-block: land on the pad and write the
                // message into the pad's local (the `problem` binding) —
                // exactly what `Term::Fail` does for explicit failures.
                if let Some(m) = msg {
                    if let Some(l) =
                        f.catch_pads.iter().find(|(b, _)| *b == next).map(|(_, l)| *l)
                    {
                        locals[l.0] = Value::Text(m);
                    }
                }
                pc = next;
                continue;
            }
            // The instructions finished — run the terminator.
            // 10.4's drain point: objects dropped mid-block (an overwritten
            // binding, a dead temporary) get their finalizers run here, while
            // this frame's locals are still alive — so a deinit body's own
            // drops chain in order, and a finalizer never re-enters the very
            // statement that dropped its object.
            self.drain_deinits(functions);
            let term = block.term.clone();
            match term {
                Term::Goto(t) => pc = t,
                Term::Branch { cond, then, otherwise } => {
                    let c = self.read(&cond, &locals);
                    match c {
                        Value::Boolean(b) => pc = if b { then } else { otherwise },
                        other => {
                            return Err(Exit::Trap(Trap {
                                message: format!(
                                    "a condition must be true or false, but this is {}.",
                                    other.format()
                                ),
                                span: Span::default(),
                            }))
                        }
                    }
                }
                Term::Break { target } => pc = target,
                Term::Continue { target } => pc = target,
                Term::Return { value } => {
                    let v = self.read(&value, &locals);
                    // The script body's final scope is the REPL's teaching
                    // data (26.4): one snapshot per run, temporaries out.
                    if f.is_entry {
                        self.entry_scope = f
                            .locals
                            .iter()
                            .enumerate()
                            .filter(|(_, l)| !l.name.starts_with('%'))
                            .map(|(i, l)| {
                                let lv = locals
                                    .get(i)
                                    .cloned()
                                    .unwrap_or(Value::Nothing)
                                    .format();
                                (l.name.clone(), l.ty.clone(), lv)
                            })
                            .collect();
                    }
                    return Err(Exit::Return(v));
                }
                Term::Fail { message, catch, .. } => {
                    let m = self.read(&message, &locals);
                    let m = match m {
                        Value::Text(s) => s,
                        other => other.format(),
                    };
                    // LOM recording for explicit failures is owned by the
                    // EventFail probe the instrument pass inserted before
                    // this terminator (26.5) — recording here too would
                    // double-count every fail.
                    match catch {
                        Some(pad_id) => {
                            // Land on the pad: write the caught message into
                            // the pad's local (the `problem` binding).
                            if let Some(l) =
                                f.catch_pads.iter().find(|(b, _)| *b == pad_id).map(|(_, l)| *l)
                            {
                                locals[l.0] = Value::Text(m);
                            }
                            pc = pad_id;
                        }
                        None => return Err(Exit::Fail(m)),
                    }
                }
                Term::Unreachable => {
                    // The verifier rejects this in finished programs.
                    return Err(Exit::Trap(Trap {
                        message: "internal: reached an unterminated block".to_string(),
                        span: Span::default(),
                    }));
                }
            }
        }
    }

    /// Read an operand. Locals are in range by construction (the verifier
    /// checks id density), so this cannot fail.
    fn read(&self, op: &Operand, locals: &[Value]) -> Value {
        match op {
            Operand::Int(v) => Value::Number(*v),
            Operand::Float(v) => Value::Decimal(*v),
            Operand::Text(s) => Value::Text(s.clone()),
            Operand::Bool(b) => Value::Boolean(*b),
            Operand::Nothing => Value::Nothing,
            Operand::Local(id) => locals.get(id.0).cloned().unwrap_or(Value::Nothing),
        }
    }

    fn record_fail(&mut self, message: &str) {
        if let Some(ring) = &mut self.ring {
            ring.push(LomEvent::Failure { message: message.to_string() });
        }
    }

    fn record_entry(&mut self, function: &str, args: &[(String, String)]) {
        if let Some(ring) = &mut self.ring {
            ring.push(LomEvent::FunctionEntry {
                function: function.to_string(),
                args: args.to_vec(),
            });
        }
    }

    fn record_bind(&mut self, name: &str, value: &str, site: usize) {
        if let Some(ring) = &mut self.ring {
            ring.push(LomEvent::Bind { name: name.to_string(), value: value.to_string(), site });
        }
    }

    // ----- the instruction step -----

    #[allow(clippy::too_many_lines)]
    fn exec_instr(
        &mut self,
        instr: &Instr,
        _f: &MirFunction,
        locals: &mut Vec<Value>,
        functions: &HashMap<&str, &MirFunction>,
        pad: Option<BlockId>,
    ) -> Step {
        match instr {
            Instr::Copy { dest, value, .. } => {
                let v = self.read(value, locals);
                // Dev LOM: binding provenance is recorded by the EventBind
                // probes the instrument pass inserted (26.5) — the Copy
                // itself stays silent, or every bind would be recorded twice.
                locals[dest.0] = v;
                Step::Continue
            }
            Instr::Unary { dest, op, operand, span } => {
                let v = self.read(operand, locals);
                locals[dest.0] = match (op, v) {
                    (UnOp::Neg, Value::Number(n)) => match n.checked_neg() {
                        Some(x) => Value::Number(x),
                        None => return Step::Exit(Exit::Trap(overflow(*span))),
                    },
                    (UnOp::Neg, Value::Decimal(d)) => Value::Decimal(-d),
                    (UnOp::Not, Value::Boolean(b)) => Value::Boolean(!b),
                    (_, v) => {
                        return Step::Exit(Exit::Trap(Trap {
                            message: format!("this operation cannot apply to {}.", v.format()),
                            span: *span,
                        }))
                    }
                };
                Step::Continue
            }
            Instr::Binary { dest, op, left, right, span } => {
                let l = self.read(left, locals);
                let r = self.read(right, locals);
                match binop(*op, l, r, *span) {
                    Ok(v) => {
                        locals[dest.0] = v;
                        Step::Continue
                    }
                    Err(e) => Step::Exit(e),
                }
            }
            Instr::Call { dest, callee, args, .. } => {
                let mut vals = Vec::with_capacity(args.len());
                for a in args {
                    vals.push(self.read(a, locals));
                }
                match self.call_user(callee, vals, functions) {
                    CallResult::Done => {
                        locals[dest.0] = Value::Nothing;
                        Step::Continue
                    }
                    CallResult::Value(v) => {
                        locals[dest.0] = v;
                        Step::Continue
                    }
                    CallResult::Fail(msg) => {
                        self.record_fail(&msg);
                        route_fail(msg, pad)
                    }
                    CallResult::Exit(e) => Step::Exit(e),
                }
            }
            Instr::Say { value, .. } => {
                let v = self.read(value, locals);
                self.host.stdout.push(v.format());
                Step::Continue
            }
            Instr::Ask { dest, question, span } => {
                let q = self.read(question, locals);
                // The prompt prints (7.1's visible question), then one stdin
                // line is consumed.
                self.host.stdout.push(q.format());
                match self.host.stdin.pop_front_line() {
                    Some(line) => {
                        locals[dest.0] = Value::Text(line);
                        Step::Continue
                    }
                    None => Step::Exit(Exit::Trap(Trap {
                        message: "`ask` reached the end of input.".to_string(),
                        span: *span,
                    })),
                }
            }
            Instr::StructNew { dest, name, fields, .. } => {
                let mut vals = Vec::with_capacity(fields.len());
                for (_, op) in fields {
                    vals.push(self.read(op, locals));
                }
                locals[dest.0] = Value::Struct { name: name.clone(), fields: vals };
                Step::Continue
            }
            Instr::ObjectNew { dest, name, fields, .. } => {
                let mut vals = Vec::with_capacity(fields.len());
                for (_, op) in fields {
                    vals.push(self.read(op, locals));
                }
                // 10.4: the finalizer hook rides the object. The hook keeps
                // the storage alive; when the LAST outer handle drops, the
                // hook's Drop queues `(class, storage)` for the VM's drain.
                let fields_rc = std::rc::Rc::new(std::cell::RefCell::new(vals));
                let hook = std::rc::Rc::new(DropHook {
                    name: name.clone(),
                    fields: fields_rc.clone(),
                    owner: self.id,
                });
                locals[dest.0] = Value::Object {
                    name: name.clone(),
                    fields: fields_rc,
                    on_drop: Some(hook),
                };
                Step::Continue
            }
            Instr::FieldGet { dest, base, field, field_span, .. } => {
                let b = self.read(base, locals);
                match b {
                    Value::Struct { fields, name } => match self.field_index(&name, field) {
                        Some(i) => {
                            locals[dest.0] = fields.into_iter().nth(i).unwrap_or(Value::Nothing);
                            Step::Continue
                        }
                        None => Step::Exit(Exit::Trap(Trap {
                            message: format!("`{name}` has no field `{field}`."),
                            span: *field_span,
                        })),
                    },
                    // A class reference reads through the shared object.
                    Value::Object { name, fields, .. } => match self.field_index(&name, field) {
                        Some(i) => {
                            locals[dest.0] = fields.borrow().get(i).cloned().unwrap_or(Value::Nothing);
                            Step::Continue
                        }
                        None => Step::Exit(Exit::Trap(Trap {
                            message: format!("`{name}` has no field `{field}`."),
                            span: *field_span,
                        })),
                    },
                    other => Step::Exit(Exit::Trap(Trap {
                        message: format!(
                            "`of` reads a structure's field, but this is {}.",
                            other.format()
                        ),
                        span: *field_span,
                    })),
                }
            }
            Instr::FieldSet { base, field, value, span, .. } => {
                // Structural mutation needs the base *local* (a value read
                // would mutate a copy). Place paths lower to base locals.
                let base_id = match base {
                    Operand::Local(id) => *id,
                    other => {
                        let v = self.read(other, locals);
                        return Step::Exit(Exit::Trap(Trap {
                            message: format!("cannot assign into {}.", v.format()),
                            span: *span,
                        }));
                    }
                };
                let v = self.read(value, locals);
                match &mut locals[base_id.0] {
                    Value::Struct { fields, name } => match self.field_index(name, field) {
                        Some(i) => {
                            fields[i] = v;
                            Step::Continue
                        }
                        None => Step::Exit(Exit::Trap(Trap {
                            message: format!("`{name}` has no field `{field}`."),
                            span: *span,
                        })),
                    },
                    // A class reference writes through the shared object: the
                    // local holds a clone of the Rc, so every alias sees the
                    // new field value (9.3's reference semantics).
                    Value::Object { name, fields, .. } => {
                        let fields = fields.clone();
                        let name = name.clone();
                        match self.field_index(&name, field) {
                            Some(i) => {
                                let mut cells = fields.borrow_mut();
                                if i < cells.len() {
                                    cells[i] = v;
                                    Step::Continue
                                } else {
                                    Step::Exit(Exit::Trap(Trap {
                                        message: format!("`{name}` has no field `{field}`."),
                                        span: *span,
                                    }))
                                }
                            }
                            None => Step::Exit(Exit::Trap(Trap {
                                message: format!("`{name}` has no field `{field}`."),
                                span: *span,
                            })),
                        }
                    }
                    other => Step::Exit(Exit::Trap(Trap {
                        message: format!(
                            "`of` assigns a structure's field, but this is {}.",
                            other.format()
                        ),
                        span: *span,
                    })),
                }
            }
            Instr::IndexGet { dest, base, index, span } => {
                let b = self.read(base, locals);
                let i = self.read(index, locals);
                match (b, i) {
                    (Value::List(items), Value::Number(n)) => match index_of(items.len(), n, *span)
                    {
                        Ok(ix) => {
                            locals[dest.0] = items[ix].clone();
                            Step::Continue
                        }
                        Err(t) => route_index_error(t, pad),
                    },
                    (Value::Text(s), Value::Number(n)) => {
                        // §7.7: `greeting at 2` — the Unicode code point by
                        // position (0-based, like list indexing).
                        let chars: Vec<char> = s.chars().collect();
                        match index_of(chars.len(), n, *span) {
                            Ok(ix) => {
                                locals[dest.0] = Value::Text(chars[ix].to_string());
                                Step::Continue
                            }
                            Err(t) => route_index_error(t, pad),
                        }
                    }
                    (Value::Map(entries), key) => {
                        // A map read of a missing key gives `nothing`
                        // (S-13/D-34's option-shaped absence).
                        locals[dest.0] = entries
                            .iter()
                            .find(|(k, _)| values_equal(k, &key))
                            .map(|(_, v)| v.clone())
                            .unwrap_or(Value::Nothing);
                        Step::Continue
                    }
                    (b, i) => Step::Exit(Exit::Trap(Trap {
                        message: format!(
                            "`at` reads a list, a map, or text, but {} at {} is not one.",
                            b.format(),
                            i.format()
                        ),
                        span: *span,
                    })),
                }
            }
            Instr::IndexSet { base, index, value, span } => {
                let base_id = match base {
                    Operand::Local(id) => *id,
                    other => {
                        let v = self.read(other, locals);
                        return Step::Exit(Exit::Trap(Trap {
                            message: format!("cannot assign into {}.", v.format()),
                            span: *span,
                        }));
                    }
                };
                let i = self.read(index, locals);
                let v = self.read(value, locals);
                match (&mut locals[base_id.0], i) {
                    (Value::List(items), Value::Number(n)) => match index_of(items.len(), n, *span)
                    {
                        Ok(ix) => {
                            items[ix] = v;
                            Step::Continue
                        }
                        Err(t) => route_index_error(t, pad),
                    },
                    (Value::Map(entries), key) => {
                        match entries.iter_mut().find(|(k, _)| values_equal(k, &key)) {
                            Some(slot) => slot.1 = v,
                            None => entries.push((key, v)),
                        }
                        Step::Continue
                    }
                    (b, i) => Step::Exit(Exit::Trap(Trap {
                        message: format!(
                            "`at` assigns into a list or a map, but {} at {} is not one.",
                            b.format(),
                            i.format()
                        ),
                        span: *span,
                    })),
                }
            }
            Instr::ListNew { dest, elements, .. } => {
                let mut vals = Vec::with_capacity(elements.len());
                for e in elements {
                    vals.push(self.read(e, locals));
                }
                locals[dest.0] = Value::List(vals);
                Step::Continue
            }
            Instr::MapNew { dest, entries, .. } => {
                let mut kvs = Vec::with_capacity(entries.len());
                for (k, v) in entries {
                    let kv = self.read(k, locals);
                    let vv = self.read(v, locals);
                    kvs.push((kv, vv));
                }
                locals[dest.0] = Value::Map(kvs);
                Step::Continue
            }
            Instr::PairNew { dest, first, second, .. } => {
                let a = self.read(first, locals);
                let b = self.read(second, locals);
                locals[dest.0] = Value::Pair(Box::new(a), Box::new(b));
                Step::Continue
            }
            Instr::Format { dest, parts, .. } => {
                let mut out = String::new();
                for p in parts {
                    match p {
                        FormatPart::Lit(s) => out.push_str(s),
                        FormatPart::Value(op) => {
                            let v = self.read(op, locals);
                            let _ = write!(out, "{}", v.format());
                        }
                    }
                }
                locals[dest.0] = Value::Text(out);
                Step::Continue
            }
            Instr::Convert { dest, conv, value, .. } => {
                let v = self.read(value, locals);
                match convert(*conv, v) {
                    ConvertOutcome::Value(v) => {
                        locals[dest.0] = v;
                        Step::Continue
                    }
                    ConvertOutcome::Fail(msg) => {
                        self.record_fail(&msg);
                        route_fail(msg, pad)
                    }
                }
            }
            Instr::Random { dest, lo, hi, span } => {
                let l = self.read(lo, locals);
                let h = self.read(hi, locals);
                match (l, h) {
                    (Value::Number(lo), Value::Number(hi)) => {
                        if hi < lo {
                            Step::Exit(Exit::Trap(Trap {
                                message: format!(
                                    "`random from {lo} to {hi}` has its bounds reversed."
                                ),
                                span: *span,
                            }))
                        } else {
                            let span_len = (hi - lo) as u64 + 1;
                            let r = self.host.next_random() % span_len;
                            locals[dest.0] = Value::Number(lo + r as i64);
                            Step::Continue
                        }
                    }
                    _ => Step::Exit(Exit::Trap(Trap {
                        message: "`random from … to …` needs two numbers.".to_string(),
                        span: *span,
                    })),
                }
            }
            Instr::FirstOf { dest, list, span } => {
                let l = self.read(list, locals);
                match l {
                    Value::List(items) => {
                        locals[dest.0] = items.into_iter().next().unwrap_or(Value::Nothing);
                        Step::Continue
                    }
                    other => Step::Exit(Exit::Trap(Trap {
                        message: format!("`first of` needs a list, but this is {}.", other.format()),
                        span: *span,
                    })),
                }
            }
            Instr::SizeOf { dest, value, span } => {
                let v = self.read(value, locals);
                locals[dest.0] = match v {
                    Value::List(items) => Value::Number(items.len() as i64),
                    Value::Map(entries) => Value::Number(entries.len() as i64),
                    Value::Text(s) => Value::Number(s.chars().count() as i64),
                    other => {
                        return Step::Exit(Exit::Trap(Trap {
                            message: format!(
                                "`size of` needs a list, map, or text, but this is {}.",
                                other.format()
                            ),
                            span: *span,
                        }))
                    }
                };
                Step::Continue
            }
            Instr::Join { dest, list, span } => {
                let l = self.read(list, locals);
                match l {
                    Value::List(items) => {
                        let parts: Vec<String> = items.iter().map(Value::format).collect();
                        locals[dest.0] = Value::Text(parts.join(""));
                        Step::Continue
                    }
                    other => Step::Exit(Exit::Trap(Trap {
                        message: format!(
                            "`join` needs a list of text, but this is {}.",
                            other.format()
                        ),
                        span: *span,
                    })),
                }
            }
            Instr::TextOp { dest, op, value, span } => {
                let v = self.read(value, locals);
                match v {
                    Value::Text(s) => {
                        locals[dest.0] = Value::Text(match op {
                            TextOp::Uppercase => s.to_uppercase(),
                            TextOp::Lowercase => s.to_lowercase(),
                            TextOp::Trim => s.trim().to_string(),
                        });
                        Step::Continue
                    }
                    other => Step::Exit(Exit::Trap(Trap {
                        message: format!(
                            "this text operation needs text, but got {}.",
                            other.format()
                        ),
                        span: *span,
                    })),
                }
            }
            Instr::MathOp { dest, op, value, span } => {
                let v = self.read(value, locals);
                locals[dest.0] = match (op, v) {
                    (MathOp::SquareRoot, Value::Number(n)) => Value::Decimal((n as f64).sqrt()),
                    (MathOp::SquareRoot, Value::Decimal(d)) => Value::Decimal(d.sqrt()),
                    (MathOp::Floor, Value::Decimal(d)) => Value::Number(d.floor() as i64),
                    (MathOp::Floor, Value::Number(n)) => Value::Number(n),
                    (_, other) => {
                        return Step::Exit(Exit::Trap(Trap {
                            message: format!(
                                "this math operation needs a number or decimal, but got {}.",
                                other.format()
                            ),
                            span: *span,
                        }))
                    }
                };
                Step::Continue
            }
            Instr::Check { value, cmp, span } => {
                let v = self.read(value, locals);
                if matches!(v, Value::Boolean(true)) {
                    return Step::Continue;
                }
                // G-4: render the comparison, naming both sides.
                let detail = match cmp {
                    Some((op, l, r)) => {
                        let lv = self.read(l, locals);
                        let rv = self.read(r, locals);
                        format!(
                            "check failed: {} (left = {}, right = {})",
                            cmp_text(*op),
                            lv.format(),
                            rv.format()
                        )
                    }
                    None => "check failed".to_string(),
                };
                Step::Exit(Exit::Trap(Trap { message: detail, span: *span }))
            }

            // ----- LOM probes: record and continue (dev only; release has
            // none — the instrument pass strips them) -----
            // ----- M1 instructions (kinds/match, options, closures,
            // combinators, files, JSON — 7.12/8.5/11/19.1) -----
            Instr::VariantTag { dest, value, .. } => {
                // The scrutinee's variant name: a struct value named by the
                // variant; the `nothing` sentinel is the option tag (8.5);
                // any other value IS `something` (D-34's value-or-nothing).
                let v = self.read(value, locals);
                let tag = match v {
                    Value::Struct { name, .. } => name,
                    Value::Nothing => "nothing".to_string(),
                    _ => "something".to_string(),
                };
                locals[dest.0] = Value::Text(tag);
                Step::Continue
            }
            Instr::PairGet { dest, pair, second, span } => {
                let p = self.read(pair, locals);
                match p {
                    Value::Pair(a, b) => {
                        locals[dest.0] = if *second { *b } else { *a };
                        Step::Continue
                    }
                    other => Step::Exit(Exit::Trap(Trap {
                        message: format!(
                            "this destructure needs a pair, but this is {}.",
                            other.format()
                        ),
                        span: *span,
                    })),
                }
            }
            Instr::MakeClosure { dest, function, captures, .. } => {
                let mut caps = Vec::with_capacity(captures.len());
                for c in captures {
                    caps.push(self.read(c, locals));
                }
                locals[dest.0] = Value::Closure { function: function.clone(), captures: caps };
                Step::Continue
            }
            Instr::CallClosure { dest, f, args, span } => {
                let fv = self.read(f, locals);
                let mut vals = Vec::with_capacity(args.len());
                for a in args {
                    vals.push(self.read(a, locals));
                }
                match fv {
                    Value::Closure { function, captures } => {
                        // The uniform closure ABI: one args list
                        // `[captures…, formals…]` is the synthetic
                        // function's single parameter.
                        // Uniform closure ABI: formals first, captures after.
                        let mut packed = vals;
                        packed.extend(captures);
                        let list = Value::List(packed);
                        match self.call_user(&function, vec![list], functions) {
                            CallResult::Done => {
                                locals[dest.0] = Value::Nothing;
                                Step::Continue
                            }
                            CallResult::Value(v) => {
                                locals[dest.0] = v;
                                Step::Continue
                            }
                            CallResult::Fail(msg) => {
                                self.record_fail(&msg);
                                route_fail(msg, pad)
                            }
                            CallResult::Exit(e) => Step::Exit(e),
                        }
                    }
                    other => Step::Exit(Exit::Trap(Trap {
                        message: format!(
                            "`call` applies a function value, but this is {}.",
                            other.format()
                        ),
                        span: *span,
                    })),
                }
            }
            // 14.2: spawn queues the closure; the join (DrainTasks) runs it.
            Instr::SpawnTask { f, keep_going, span } => {
                let fv = self.read(f, locals);
                match fv {
                    Value::Closure { .. } => {
                        self.tasks.push((fv, *keep_going));
                        Step::Continue
                    }
                    other => Step::Exit(Exit::Trap(Trap {
                        message: format!(
                            "`start a task` needs a task body, but got {}.",
                            other.format()
                        ),
                        span: *span,
                    })),
                }
            }
            // 14.2: the join — run every queued task to completion in spawn
            // order, then re-raise the first failure that supervision did not
            // consume (`keep going`). Deterministic: one backend order, both
            // backends share it.
            Instr::DrainTasks { .. } => match self.drain_tasks(functions) {
                Ok(()) => Step::Continue,
                Err(msg) => {
                    self.record_fail(&msg);
                    route_fail(msg, pad)
                }
            },
            // 14.3: send queues a deep copy (values cross the boundary).
            Instr::SendChannel { value, channel, span } => {
                let v = self.read(value, locals);
                let c = self.read(channel, locals);
                match c {
                    Value::Channel(q) => {
                        q.borrow_mut().push(v);
                        Step::Continue
                    }
                    other => Step::Exit(Exit::Trap(Trap {
                        message: format!(
                            "`send … to` needs a channel, but this is {}.",
                            other.format()
                        ),
                        span: *span,
                    })),
                }
            }
            // 14.3: receive dequeues; on empty, the join barrier runs first
            // (waiting for the region's remaining tasks), then still-empty
            // fails — `receive from an empty channel`.
            Instr::ReceiveChannel { dest, channel, span } => {
                let c = self.read(channel, locals);
                match c {
                    Value::Channel(q) => {
                        if q.borrow().is_empty() {
                            // The empty-wait rule: join the region's remaining
                            // tasks, then look again.
                            match self.drain_tasks(functions) {
                                Ok(()) => {}
                                Err(msg) => {
                                    self.record_fail(&msg);
                                    return route_fail(msg, pad);
                                }
                            }
                        }
                        let next = q.borrow_mut().pop();
                        match next {
                            Some(v) => {
                                locals[dest.0] = v;
                                Step::Continue
                            }
                            None => {
                                let msg = String::from("receive from an empty channel");
                                self.record_fail(&msg);
                                route_fail(msg, pad)
                            }
                        }
                    }
                    other => Step::Exit(Exit::Trap(Trap {
                        message: format!(
                            "`receive from` needs a channel, but this is {}.",
                            other.format()
                        ),
                        span: *span,
                    })),
                }
            }
            // 14.3: the channel construction.
            Instr::NewChannel { dest, .. } => {
                locals[dest.0] = Value::Channel(Default::default());
                Step::Continue
            }
            Instr::MapList { dest, list, f, span }
            | Instr::KeepList { dest, list, f, span } => {
                let l = self.read(list, locals);
                let fv = self.read(f, locals);
                let is_map = matches!(instr, Instr::MapList { .. });
                match (l, fv) {
                    (Value::List(items), fv @ Value::Closure { .. }) => {
                        let mut out = Vec::with_capacity(items.len());
                        for item in items {
                            match self.call_closure_1(&fv, item.clone(), functions, pad) {
                                Ok(v) => {
                                    // `keep` filters on truthiness of the
                                    // predicate's boolean (11.2).
                                    if is_map {
                                        out.push(v);
                                    } else if matches!(v, Value::Boolean(true)) {
                                        out.push(item);
                                    }
                                }
                                Err(e) => return Step::Exit(e),
                            }
                        }
                        locals[dest.0] = Value::List(out);
                        Step::Continue
                    }
                    (Value::List(_), other) => Step::Exit(Exit::Trap(Trap {
                        message: format!(
                            "`{}` needs a function value, but got {}.",
                            if is_map { "map" } else { "keep" },
                            other.format()
                        ),
                        span: *span,
                    })),
                    (other, _) => Step::Exit(Exit::Trap(Trap {
                        message: format!(
                            "`{}` needs a list, but this is {}.",
                            if is_map { "map" } else { "keep" },
                            other.format()
                        ),
                        span: *span,
                    })),
                }
            }
            Instr::CombineList { dest, list, start, f, span } => {
                let l = self.read(list, locals);
                let acc0 = self.read(start, locals);
                let fv = self.read(f, locals);
                match (l, fv) {
                    (Value::List(items), fv @ Value::Closure { .. }) => {
                        let mut acc = acc0;
                        for item in items {
                            match self.call_closure_2(&fv, acc, item, functions, pad) {
                                Ok(v) => acc = v,
                                Err(e) => return Step::Exit(e),
                            }
                        }
                        locals[dest.0] = acc;
                        Step::Continue
                    }
                    (Value::List(_), other) => Step::Exit(Exit::Trap(Trap {
                        message: format!(
                            "`combine` needs a function value, but got {}.",
                            other.format()
                        ),
                        span: *span,
                    })),
                    (other, _) => Step::Exit(Exit::Trap(Trap {
                        message: format!("`combine` needs a list, but this is {}.", other.format()),
                        span: *span,
                    })),
                }
            }
            Instr::SplitText { dest, text, sep, span } => {
                let t = self.read(text, locals);
                let s = self.read(sep, locals);
                match (t, s) {
                    (Value::Text(t), Value::Text(sep)) => {
                        if sep.is_empty() {
                            Step::Exit(Exit::Trap(Trap {
                                message: "`split` needs a non-empty separator.".to_string(),
                                span: *span,
                            }))
                        } else {
                            let parts = t.split(sep.as_str()).map(|p| Value::Text(p.to_string())).collect();
                            locals[dest.0] = Value::List(parts);
                            Step::Continue
                        }
                    }
                    _ => Step::Exit(Exit::Trap(Trap {
                        message: "`split` needs two texts.".to_string(),
                        span: *span,
                    })),
                }
            }
            Instr::SortList { dest, list, span } => {
                let l = self.read(list, locals);
                match l {
                    Value::List(mut items) => {
                        let sorted = items.sort_by(|a, b| value_order(a, b));
                        let _ = sorted;
                        locals[dest.0] = Value::List(items);
                        Step::Continue
                    }
                    other => Step::Exit(Exit::Trap(Trap {
                        message: format!("`sort` needs a list, but this is {}.", other.format()),
                        span: *span,
                    })),
                }
            }
            Instr::JsonParse { dest, text, span } => {
                let t = self.read(text, locals);
                match t {
                    Value::Text(s) => match json_parse(s.trim()) {
                        Ok(v) => {
                            locals[dest.0] = v;
                            Step::Continue
                        }
                        Err(msg) => {
                            let msg = format!("this text is not valid JSON: {msg}");
                            self.record_fail(&msg);
                            route_fail(msg, pad)
                        }
                    },
                    other => Step::Exit(Exit::Trap(Trap {
                        message: format!(
                            "`json from` needs text, but got {}.",
                            other.format()
                        ),
                        span: *span,
                    })),
                }
            }
            Instr::JsonFormat { dest, value, .. } => {
                let v = self.read(value, locals);
                locals[dest.0] = Value::Text(json_format(&v));
                Step::Continue
            }
            Instr::FileOp { dest, op, a, b, span } => {
                match self.file_op(*op, *dest, a, b.as_ref(), locals, *span, pad) {
                    Ok(()) => Step::Continue,
                    Err(FileOutcome::Fail(msg)) => {
                        self.record_fail(&msg);
                        route_fail(msg, pad)
                    }
                    Err(FileOutcome::Trap(t)) => Step::Exit(Exit::Trap(t)),
                }
            }

            Instr::EventFunctionEntry { function, args } => {
                if self.dev {
                    let mut rendered: Vec<(String, String)> = Vec::with_capacity(args.len());
                    for (n, op) in args {
                        let v = self.read(op, locals);
                        rendered.push((n.clone(), v.format()));
                    }
                    self.record_entry(function, &rendered);
                }
                Step::Continue
            }
            Instr::EventBind { name, value, site } => {
                if self.dev {
                    let v = self.read(value, locals);
                    self.record_bind(name, &v.format(), site.start);
                }
                Step::Continue
            }
            Instr::EventFail { message } => {
                if self.dev {
                    let m = self.read(message, locals);
                    self.record_fail(&m.format());
                }
                Step::Continue
            }
        }
    }

    fn field_index(&self, struct_name: &str, field: &str) -> Option<usize> {
        self.struct_fields.get(struct_name)?.iter().position(|f| f == field)
    }

    /// Call a closure value with one argument (the combinators' element
    /// lambda). A failing closure body propagates like any call (13.1).
    fn call_closure_1(
        &mut self,
        f: &Value,
        arg: Value,
        functions: &HashMap<&str, &MirFunction>,
        pad: Option<BlockId>,
    ) -> Result<Value, Exit> {
        let Value::Closure { function, captures } = f else {
            return Err(Exit::Trap(Trap {
                message: "internal: combinator called with a non-closure".to_string(),
                span: Span::default(),
            }));
        };
        let mut packed = vec![arg];
        packed.extend(captures.clone());
        match self.call_user(function, vec![Value::List(packed)], functions) {
            CallResult::Done => Ok(Value::Nothing),
            CallResult::Value(v) => Ok(v),
            CallResult::Fail(msg) => {
                self.record_fail(&msg);
                match pad {
                    // Inside an attempt the failure composes (13.1); in a
                    // combinator loop with no pad it exits the frame.
                    Some(_) => Ok(Value::Nothing),
                    None => Err(Exit::Fail(msg)),
                }
            }
            CallResult::Exit(e) => Err(e),
        }
    }

    /// Call a closure value with two arguments (`combine`'s
    /// (accumulator, element) step — 11.2's fold shape).
    fn call_closure_2(
        &mut self,
        f: &Value,
        a: Value,
        b: Value,
        functions: &HashMap<&str, &MirFunction>,
        pad: Option<BlockId>,
    ) -> Result<Value, Exit> {
        let Value::Closure { function, captures } = f else {
            return Err(Exit::Trap(Trap {
                message: "internal: combinator called with a non-closure".to_string(),
                span: Span::default(),
            }));
        };
        let mut packed = vec![a, b];
        packed.extend(captures.clone());
        match self.call_user(function, vec![Value::List(packed)], functions) {
            CallResult::Done => Ok(Value::Nothing),
            CallResult::Value(v) => Ok(v),
            CallResult::Fail(msg) => {
                self.record_fail(&msg);
                match pad {
                    Some(_) => Ok(Value::Nothing),
                    None => Err(Exit::Fail(msg)),
                }
            }
            CallResult::Exit(e) => Err(e),
        }
    }

    /// One file operation (§19.1). Paths resolve inside `Host::workdir`;
    /// every operation except `exists` can fail (§13.1), and the failure
    /// value is the message text (S-10).
    #[allow(clippy::too_many_arguments)]
    fn file_op(
        &mut self,
        op: FileOp,
        dest: LocalId,
        a: &Operand,
        b: Option<&Operand>,
        locals: &mut Vec<Value>,
        span: Span,
        _pad: Option<BlockId>,
    ) -> Result<(), FileOutcome> {
        let path_v = self.read(a, locals);
        let Value::Text(path_s) = path_v else {
            return Err(FileOutcome::Trap(Trap {
                message: format!("file operations need text paths, but got {}.", path_v.format()),
                span,
            }));
        };
        // `write file <content> at <path>` carries the content first (the
        // call's first argument); every other op's first argument is the
        // path. Resolve the path per-op so both spellings hit the same
        // workdir rule.
        let write_content = if matches!(op, FileOp::Write | FileOp::Append) {
            let content_v = self.read(a, locals);
            let Value::Text(content_s) = content_v else {
                return Err(FileOutcome::Trap(Trap {
                    message: format!(
                        "file writes need text, but got {}.",
                        content_v.format()
                    ),
                    span,
                }));
            };
            let Some(bop) = b else {
                return Err(FileOutcome::Trap(Trap {
                    message: "internal: a write needs a path".to_string(),
                    span,
                }));
            };
            let path_v2 = self.read(bop, locals);
            let Value::Text(path2) = path_v2 else {
                return Err(FileOutcome::Trap(Trap {
                    message: format!(
                        "file operations need text paths, but got {}.",
                        path_v2.format()
                    ),
                    span,
                }));
            };
            Some((content_s, path2))
        } else {
            None
        };
        let full = self.host.workdir.join(match &write_content {
            Some((_, p)) => p,
            None => &path_s,
        });
        match op {
            FileOp::Exists => {
                locals[dest.0] = Value::Boolean(full.exists());
                Ok(())
            }
            FileOp::Size => match std::fs::metadata(&full) {
                Ok(m) => {
                    locals[dest.0] = Value::Number(m.len() as i64);
                    Ok(())
                }
                Err(e) => Err(FileOutcome::Fail(format!(
                    "could not measure \"{}\": {}.",
                    path_s,
                    e.kind()
                ))),
            },
            FileOp::Read => match std::fs::read_to_string(&full) {
                Ok(s) => {
                    locals[dest.0] = Value::Text(s);
                    Ok(())
                }
                Err(e) => Err(FileOutcome::Fail(format!(
                    "could not open \"{}\": {}.",
                    path_s,
                    e.kind()
                ))),
            },
            FileOp::Write | FileOp::Append => {
                // The content/path pair was extracted up front (the call's
                // first argument is the *content*; `at` carries the path).
                let Some((text, path_s)) = write_content else {
                    return Err(FileOutcome::Trap(Trap {
                        message: "internal: a write needs content".to_string(),
                        span,
                    }));
                };
                let result = if op == FileOp::Write {
                    std::fs::write(&full, text.as_bytes())
                } else {
                    use std::io::Write as _;
                    std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&full)
                        .and_then(|mut f| f.write_all(text.as_bytes()))
                }
                .or_else(|e| {
                    // A write into a not-yet-existing folder creates the
                    // folder (the files module's teaching rule; the
                    // file-organizer project's shape). Identical in both
                    // backends.
                    if e.kind() == std::io::ErrorKind::NotFound {
                        if let Some(parent) = std::path::Path::new(&full).parent() {
                            if std::fs::create_dir_all(parent).is_ok() {
                                return if op == FileOp::Write {
                                    std::fs::write(&full, text.as_bytes())
                                } else {
                                    use std::io::Write as _;
                                    std::fs::OpenOptions::new()
                                        .create(true)
                                        .append(true)
                                        .open(&full)
                                        .and_then(|mut f| f.write_all(text.as_bytes()))
                                };
                            }
                        }
                    }
                    Err(e)
                });
                match result {
                    Ok(()) => {
                        locals[dest.0] = Value::Text(text);
                        Ok(())
                    }
                    Err(e) => Err(FileOutcome::Fail(format!(
                        "could not write \"{}\": {}.",
                        path_s,
                        e.kind()
                    ))),
                }
            }
            FileOp::Delete => match std::fs::remove_file(&full) {
                Ok(()) => {
                    locals[dest.0] = Value::Boolean(true);
                    Ok(())
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    // Deleting an absent file already did the job — deleting
                    // is idempotent for the student.
                    locals[dest.0] = Value::Boolean(true);
                    Ok(())
                }
                Err(e) => Err(FileOutcome::Fail(format!(
                    "could not delete \"{}\": {}.",
                    path_s,
                    e.kind()
                ))),
            },
        }
    }

    /// A user call: push a frame by recursion (M0 is sequential — 13.1's
    /// model makes direct recursion exact, and each frame's locals are
    /// fresh, which is what recursion needs).
    fn call_user(
        &mut self,
        callee: &str,
        args: Vec<Value>,
        functions: &HashMap<&str, &MirFunction>,
    ) -> CallResult {
        // 12.3/10.6 vtable dispatch: `iface <I> <m>` resolves through the
        // receiver's runtime class (arg 0) to the implementing callee —
        // own method, inherited, or copied default — the same resolution
        // sema's table encodes. Missing receiver/row is an internal error:
        // the checker proved the call real before emitting it.
        let callee: &str = match callee.strip_prefix("iface ") {
            Some(rest) => {
                let (iname, m) = rest.split_once(' ').unwrap_or((rest, ""));
                let class = match args.first() {
                    Some(Value::Object { name, .. }) => name.clone(),
                    Some(v) => {
                        return CallResult::Exit(Exit::Trap(Trap {
                            message: format!(
                                "internal: `iface {iname} {m}` dispatched on a non-object value ({v:?})."
                            ),
                            span: Span::default(),
                        }));
                    }
                    None => {
                        return CallResult::Exit(Exit::Trap(Trap {
                            message: format!(
                                "internal: `iface {iname} {m}` dispatched with no receiver."
                            ),
                            span: Span::default(),
                        }));
                    }
                };
                match self
                    .iface_dispatch
                    .get(iname)
                    .and_then(|t| t.get(&class))
                {
                    Some(target) => target.as_str(),
                    None => {
                        return CallResult::Exit(Exit::Trap(Trap {
                            message: format!(
                                "internal: `{class}` does not implement `{m}` for interface `{iname}`."
                            ),
                            span: Span::default(),
                        }));
                    }
                }
            }
            None => callee,
        };
        let Some(f) = functions.get(callee) else {
            return CallResult::Exit(Exit::Trap(Trap {
                message: format!("`{callee}` is not a function this program defines."),
                span: Span::default(),
            }));
        };
        let f: &MirFunction = f;
        match self.call_function(f, &args, functions) {
            Ok(()) => CallResult::Done,
            Err(Exit::Return(v)) => {
                if matches!(v, Value::Nothing) && f.ret.is_none() {
                    CallResult::Done
                } else {
                    CallResult::Value(v)
                }
            }
            Err(Exit::Fail(m)) => CallResult::Fail(m),
            Err(Exit::Trap(t)) => CallResult::Exit(Exit::Trap(t)),
        }
    }
}

enum Step {
    Continue,
    /// Jump to a block; the `Some(message)` form carries an instruction-level
    /// failure's error to the pad's local (written on landing).
    Jump(BlockId, Option<String>),
    Exit(Exit),
}

enum CallResult {
    Done,
    Value(Value),
    Fail(String),
    Exit(Exit),
}

enum ConvertOutcome {
    Value(Value),
    Fail(String),
}

/// A file operation's outcome (§19.1): ok, a failure (catchable), or a trap
/// (a misuse — wrong types — is a crash, not an error value).
enum FileOutcome {
    Fail(String),
    Trap(Trap),
}

fn route_fail(msg: String, pad: Option<BlockId>) -> Step {
    match pad {
        Some(p) => Step::Jump(p, Some(msg)),
        None => Step::Exit(Exit::Fail(msg)),
    }
}

/// An out-of-range index is a runtime error: inside an attempt it composes
/// with the error model (it *is* a failure — 13.1's honesty rule), outside
/// it is a crash.
fn route_index_error(t: Trap, pad: Option<BlockId>) -> Step {
    match pad {
        // Inside an attempt, an out-of-range index *is* a failure (13.1's
        // honesty rule): it flows to the pad exactly like `fail with`.
        Some(p) => Step::Jump(p, Some(t.message)),
        None => Step::Exit(Exit::Trap(t)),
    }
}

fn index_of(len: usize, n: i64, span: Span) -> Result<usize, Trap> {
    if n < 0 {
        return Err(Trap {
            message: format!("a list index cannot be negative, but this is {n}."),
            span,
        });
    }
    let n = n as usize;
    if n >= len {
        return Err(Trap {
            message: format!("index {n} is past the end of this list ({len} items)."),
            span,
        });
    }
    Ok(n)
}

/// The binop table (S-1/S-3, D-10): `Div` promotes; `DivEvenly` floors
/// (Python pair semantics); i64 arithmetic is checked (traps, 13.1).
fn binop(op: BinOp, l: Value, r: Value, span: Span) -> Result<Value, Exit> {
    use BinOp::*;
    match (op, l, r) {
        (Add, Value::Number(a), Value::Number(b)) => a
            .checked_add(b)
            .map(Value::Number)
            .ok_or_else(|| Exit::Trap(overflow(span))),
        (Add, Value::Decimal(a), Value::Decimal(b)) => Ok(Value::Decimal(a + b)),
        (Add, Value::Number(a), Value::Decimal(b)) => Ok(Value::Decimal(a as f64 + b)),
        (Add, Value::Decimal(a), Value::Number(b)) => Ok(Value::Decimal(a + b as f64)),
        (Add, Value::Text(a), Value::Text(b)) => Ok(Value::Text(a + &b)),
        (Sub, Value::Number(a), Value::Number(b)) => a
            .checked_sub(b)
            .map(Value::Number)
            .ok_or_else(|| Exit::Trap(overflow(span))),
        (Sub, Value::Decimal(a), Value::Decimal(b)) => Ok(Value::Decimal(a - b)),
        (Sub, Value::Number(a), Value::Decimal(b)) => Ok(Value::Decimal(a as f64 - b)),
        (Sub, Value::Decimal(a), Value::Number(b)) => Ok(Value::Decimal(a - b as f64)),
        (Mul, Value::Number(a), Value::Number(b)) => a
            .checked_mul(b)
            .map(Value::Number)
            .ok_or_else(|| Exit::Trap(overflow(span))),
        (Mul, Value::Decimal(a), Value::Decimal(b)) => Ok(Value::Decimal(a * b)),
        (Mul, Value::Number(a), Value::Decimal(b)) => Ok(Value::Decimal(a as f64 * b)),
        (Mul, Value::Decimal(a), Value::Number(b)) => Ok(Value::Decimal(a * b as f64)),
        // `divided by` always promotes (D-10). Decimal division by zero
        // follows IEEE infinity; the *integer* rule is the teachable trap.
        (Div, Value::Number(a), Value::Number(b)) => {
            if b == 0 {
                Err(Exit::Trap(Trap { message: "cannot divide by zero.".to_string(), span }))
            } else {
                Ok(Value::Decimal(a as f64 / b as f64))
            }
        }
        (Div, Value::Decimal(a), Value::Decimal(b)) => Ok(Value::Decimal(a / b)),
        (Div, Value::Number(a), Value::Decimal(b)) => Ok(Value::Decimal(a as f64 / b)),
        (Div, Value::Decimal(a), Value::Number(b)) => Ok(Value::Decimal(a / b as f64)),
        // `divided evenly by` floors (Python `//`): div_euclid floors toward
        // negative infinity; `remainder of` pairs with rem_euclid.
        (DivEvenly, Value::Number(a), Value::Number(b)) => {
            if b == 0 {
                Err(Exit::Trap(Trap {
                    message: "cannot divide evenly by zero.".to_string(),
                    span,
                }))
            } else {
                Ok(Value::Number(a.div_euclid(b)))
            }
        }
        (DivEvenly, Value::Decimal(a), Value::Decimal(b)) => Ok(Value::Decimal((a / b).floor())),
        (DivEvenly, Value::Number(a), Value::Decimal(b)) => {
            Ok(Value::Decimal((a as f64 / b).floor()))
        }
        (DivEvenly, Value::Decimal(a), Value::Number(b)) => Ok(Value::Decimal((a / b as f64).floor())),
        (Rem, Value::Number(a), Value::Number(b)) => {
            if b == 0 {
                Err(Exit::Trap(Trap {
                    message: "cannot take the remainder of zero.".to_string(),
                    span,
                }))
            } else {
                // Python semantics: the remainder takes the divisor's sign.
                Ok(Value::Number(a.rem_euclid(b.abs()) * b.signum()))
            }
        }
        (Rem, Value::Decimal(a), Value::Decimal(b)) => Ok(Value::Decimal(a % b)),
        (Rem, Value::Number(a), Value::Decimal(b)) => Ok(Value::Decimal(a as f64 % b)),
        (Rem, Value::Decimal(a), Value::Number(b)) => Ok(Value::Decimal(a % b as f64)),
        (And, Value::Boolean(a), Value::Boolean(b)) => Ok(Value::Boolean(a && b)),
        (Or, Value::Boolean(a), Value::Boolean(b)) => Ok(Value::Boolean(a || b)),
        // G-27: max/min of two numbers (§7.8's `bigger of a and b`).
        (Max, Value::Number(a), Value::Number(b)) => Ok(Value::Number(a.max(b))),
        (Max, Value::Decimal(a), Value::Decimal(b)) => Ok(Value::Decimal(a.max(b))),
        (Max, Value::Number(a), Value::Decimal(b)) => {
            let d = a as f64;
            if d >= b { Ok(Value::Decimal(d)) } else { Ok(Value::Decimal(b)) }
        }
        (Max, Value::Decimal(a), Value::Number(b)) => {
            let d = b as f64;
            if a >= d { Ok(Value::Decimal(a)) } else { Ok(Value::Decimal(d)) }
        }
        (Min, Value::Number(a), Value::Number(b)) => Ok(Value::Number(a.min(b))),
        (Min, Value::Decimal(a), Value::Decimal(b)) => Ok(Value::Decimal(a.min(b))),
        (Min, Value::Number(a), Value::Decimal(b)) => {
            let d = a as f64;
            if d <= b { Ok(Value::Decimal(d)) } else { Ok(Value::Decimal(b)) }
        }
        (Min, Value::Decimal(a), Value::Number(b)) => {
            let d = b as f64;
            if a <= d { Ok(Value::Decimal(a)) } else { Ok(Value::Decimal(d)) }
        }
        (Contains, l, r) => binop_contains(l, r, span),
        (op, l, r) => compare(op, l, r, span).map(Value::Boolean),
    }
}

fn overflow(span: Span) -> Trap {
    Trap {
        message: "a number grew past its largest possible value (overflow).".to_string(),
        span,
    }
}

// ---------------------------------------------------------------------------
// M1: text containment, ordering, JSON (docs/07's student set, §19.1)
// ---------------------------------------------------------------------------

fn binop_contains(l: Value, r: Value, span: Span) -> Result<Value, Exit> {
    match (l, r) {
        (Value::Text(h), Value::Text(n)) => Ok(Value::Boolean(h.contains(n.as_str()))),
        (l, r) => Err(Exit::Trap(Trap {
            message: format!(
                "`contains` needs two texts, but got {} and {}.",
                l.format(),
                r.format()
            ),
            span,
        })),
    }
}

/// A total order over list elements for `sort`: numbers (and decimals)
/// numerically, text code-point order, booleans false-first; mixed kinds
/// sort by nothing-defined-but-stable tag order rather than crashing — a
/// teaching sort stays total.
fn value_order(a: &Value, b: &Value) -> std::cmp::Ordering {
    fn rank(v: &Value) -> u8 {
        match v {
            Value::Boolean(_) => 0,
            Value::Number(_) | Value::Decimal(_) => 1,
            Value::Text(_) => 2,
            _ => 3,
        }
    }
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.cmp(y),
        (Value::Number(x), Value::Decimal(y)) => (*x as f64).partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
        (Value::Decimal(x), Value::Number(y)) => x.partial_cmp(&(*y as f64)).unwrap_or(std::cmp::Ordering::Equal),
        (Value::Decimal(x), Value::Decimal(y)) => x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
        (Value::Text(x), Value::Text(y)) => x.cmp(y),
        (Value::Boolean(x), Value::Boolean(y)) => x.cmp(y),
        _ => rank(a).cmp(&rank(b)),
    }
}

/// Parse JSON text into the M1 value surface (§19.1): objects become maps
/// (text keys), arrays lists, strings/numbers/booleans/null their Lagom
/// values (`null` is `nothing` — 8.5's sentinel).
fn json_parse(s: &str) -> Result<Value, String> {
    let bytes: Vec<char> = s.chars().collect();
    let mut pos = 0usize;
    let v = json_value(&bytes, &mut pos)?;
    json_skip_ws(&bytes, &mut pos);
    if pos != bytes.len() {
        return Err(format!("unexpected text after the value at character {pos}"));
    }
    Ok(v)
}

fn json_skip_ws(b: &[char], pos: &mut usize) {
    while *pos < b.len() && b[*pos].is_whitespace() {
        *pos += 1;
    }
}

fn json_value(b: &[char], pos: &mut usize) -> Result<Value, String> {
    json_skip_ws(b, pos);
    match b.get(*pos) {
        Some('{') => {
            *pos += 1;
            let mut entries: Vec<(Value, Value)> = Vec::new();
            json_skip_ws(b, pos);
            if b.get(*pos) == Some(&'}') {
                *pos += 1;
                return Ok(Value::Map(entries));
            }
            loop {
                json_skip_ws(b, pos);
                let key = json_string(b, pos)?;
                json_skip_ws(b, pos);
                if b.get(*pos) != Some(&':') {
                    return Err(format!("expected ':' in the object at character {pos}"));
                }
                *pos += 1;
                let val = json_value(b, pos)?;
                entries.push((Value::Text(key), val));
                json_skip_ws(b, pos);
                match b.get(*pos) {
                    Some(',') => *pos += 1,
                    Some('}') => {
                        *pos += 1;
                        return Ok(Value::Map(entries));
                    }
                    _ => return Err(format!("expected ',' or '}}' at character {pos}")),
                }
            }
        }
        Some('[') => {
            *pos += 1;
            let mut items = Vec::new();
            json_skip_ws(b, pos);
            if b.get(*pos) == Some(&']') {
                *pos += 1;
                return Ok(Value::List(items));
            }
            loop {
                let v = json_value(b, pos)?;
                items.push(v);
                json_skip_ws(b, pos);
                match b.get(*pos) {
                    Some(',') => *pos += 1,
                    Some(']') => {
                        *pos += 1;
                        return Ok(Value::List(items));
                    }
                    _ => return Err(format!("expected ',' or ']' at character {pos}")),
                }
            }
        }
        Some('"') => Ok(Value::Text(json_string(b, pos)?)),
        Some('t') => {
            json_expect(b, pos, "true")?;
            Ok(Value::Boolean(true))
        }
        Some('f') => {
            json_expect(b, pos, "false")?;
            Ok(Value::Boolean(false))
        }
        Some('n') => {
            json_expect(b, pos, "null")?;
            Ok(Value::Nothing)
        }
        Some(c) if *c == '-' || c.is_ascii_digit() => {
            let start = *pos;
            if b.get(*pos) == Some(&'-') {
                *pos += 1;
            }
            while *pos < b.len() && (b[*pos].is_ascii_digit() || b[*pos] == '.') {
                *pos += 1;
            }
            if *pos < b.len() && (b[*pos] == 'e' || b[*pos] == 'E') {
                *pos += 1;
                if *pos < b.len() && (b[*pos] == '+' || b[*pos] == '-') {
                    *pos += 1;
                }
                while *pos < b.len() && b[*pos].is_ascii_digit() {
                    *pos += 1;
                }
            }
            let text: String = b[start..*pos].iter().collect();
            if text.contains('.') || text.contains('e') || text.contains('E') {
                text.parse::<f64>()
                    .map(Value::Decimal)
                    .map_err(|_| format!("bad number \"{text}\""))
            } else {
                text.parse::<i64>()
                    .map(Value::Number)
                    .map_err(|_| format!("bad number \"{text}\""))
            }
        }
        _ => Err(format!("unexpected character at position {pos}")),
    }
}

fn json_expect(b: &[char], pos: &mut usize, word: &str) -> Result<(), String> {
    for c in word.chars() {
        if b.get(*pos) != Some(&c) {
            return Err(format!("expected '{word}' at character {pos}"));
        }
        *pos += 1;
    }
    Ok(())
}

fn json_string(b: &[char], pos: &mut usize) -> Result<String, String> {
    if b.get(*pos) != Some(&'"') {
        return Err(format!("expected a string at character {pos}"));
    }
    *pos += 1;
    let mut out = String::new();
    while let Some(&c) = b.get(*pos) {
        *pos += 1;
        match c {
            '"' => return Ok(out),
            '\\' => match b.get(*pos) {
                Some('"') => {
                    out.push('"');
                    *pos += 1;
                }
                Some('\\') => {
                    out.push('\\');
                    *pos += 1;
                }
                Some('/') => {
                    out.push('/');
                    *pos += 1;
                }
                Some('n') => {
                    out.push('\n');
                    *pos += 1;
                }
                Some('t') => {
                    out.push('\t');
                    *pos += 1;
                }
                Some('r') => {
                    out.push('\r');
                    *pos += 1;
                }
                Some('u') => {
                    *pos += 1;
                    let hex: String = b[*pos..(*pos + 4).min(b.len())].iter().collect();
                    *pos += 4;
                    let code = u32::from_str_radix(&hex, 16)
                        .map_err(|_| format!("bad \\u escape \"{hex}\""))?;
                    out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                }
                _ => return Err(format!("bad escape at character {pos}")),
            },
            c => out.push(c),
        }
    }
    Err("the string never closed".to_string())
}

/// Format a value as JSON (§19.1): maps are objects, lists arrays, and the
/// student scalars map directly. Structs format as objects of their fields
/// (field names are known to the program, not to this printer — the simple
/// positional form stays for them).
fn json_format(v: &Value) -> String {
    match v {
        Value::Number(n) => n.to_string(),
        Value::Decimal(d) => format_decimal(*d),
        Value::Text(s) => json_string_lit(s),
        Value::Boolean(b) => (if *b { "true" } else { "false" }).to_string(),
        Value::Nothing => "null".to_string(),
        Value::List(items) => {
            let inner: Vec<String> = items.iter().map(json_format).collect();
            format!("[{}]", inner.join(", "))
        }
        Value::Map(entries) => {
            let inner: Vec<String> = entries
                .iter()
                .map(|(k, v)| format!("{}: {}", json_string_lit(&k.format()), json_format(v)))
                .collect();
            format!("{{{}}}", inner.join(", "))
        }
        Value::Pair(a, b) => format!("[{}, {}]", json_format(a), json_format(b)),
        Value::Struct { name, fields } => {
            let inner: Vec<String> = fields
                .iter()
                .map(|f| format!("{}: {}", json_string_lit(name), json_format(f)))
                .collect();
            format!("{{{}}}", inner.join(", "))
        }
        Value::Object { .. } => "null".to_string(),
        Value::Closure { .. } => "null".to_string(),
        // 14.3: a channel JSON-formats as the messages still queued.
        Value::Channel(q) => {
            let inner: Vec<String> = q.borrow().iter().map(json_format).collect();
            format!("[{}]", inner.join(", "))
        }
    }
}

fn json_string_lit(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Comparisons: numbers/decimals mix and promote; text and booleans compare
/// exactly; anything else falls to structural equality for Equal/NotEqual
/// (D-30) and traps for the ordered comparisons.
fn compare(op: BinOp, l: Value, r: Value, span: Span) -> Result<bool, Exit> {
    let ord = match (&l, &r) {
        (Value::Number(a), Value::Number(b)) => Some(a.cmp(b)),
        (Value::Number(a), Value::Decimal(b)) => (*a as f64).partial_cmp(b),
        (Value::Decimal(a), Value::Number(b)) => a.partial_cmp(&(*b as f64)),
        (Value::Decimal(a), Value::Decimal(b)) => a.partial_cmp(b),
        (Value::Text(a), Value::Text(b)) => Some(a.cmp(b)),
        (Value::Boolean(a), Value::Boolean(b)) => Some(a.cmp(b)),
        _ => None,
    };
    match ord {
        Some(ord) => Ok(match op {
            BinOp::Equal => ord == std::cmp::Ordering::Equal,
            BinOp::NotEqual => ord != std::cmp::Ordering::Equal,
            BinOp::Greater => ord == std::cmp::Ordering::Greater,
            BinOp::Less => ord == std::cmp::Ordering::Less,
            BinOp::AtLeast => ord != std::cmp::Ordering::Less,
            BinOp::AtMost => ord != std::cmp::Ordering::Greater,
            _ => unreachable!("non-comparison ops never reach compare"),
        }),
        None => match op {
            BinOp::Equal => Ok(values_equal(&l, &r)),
            BinOp::NotEqual => Ok(!values_equal(&l, &r)),
            _ => Err(Exit::Trap(Trap {
                message: format!(
                    "these two values cannot be put in order: {} and {}.",
                    l.format(),
                    r.format()
                ),
                span,
            })),
        },
    }
}

/// Structural equality (D-30): same type, same contents; `Nothing` equals
/// `Nothing`; numbers and decimals compare numerically.
fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x == y,
        (Value::Number(x), Value::Decimal(y)) | (Value::Decimal(y), Value::Number(x)) => {
            *x as f64 == *y
        }
        (Value::Decimal(x), Value::Decimal(y)) => x == y,
        (Value::Text(x), Value::Text(y)) => x == y,
        (Value::Boolean(x), Value::Boolean(y)) => x == y,
        (Value::Nothing, Value::Nothing) => true,
        (Value::List(xs), Value::List(ys)) => {
            xs.len() == ys.len() && xs.iter().zip(ys).all(|(x, y)| values_equal(x, y))
        }
        (Value::Map(xs), Value::Map(ys)) => {
            xs.len() == ys.len()
                && xs
                    .iter()
                    .all(|(k, v)| ys.iter().any(|(k2, v2)| values_equal(k, k2) && values_equal(v, v2)))
        }
        (Value::Pair(a1, b1), Value::Pair(a2, b2)) => values_equal(a1, a2) && values_equal(b1, b2),
        (Value::Struct { name: n1, fields: f1 }, Value::Struct { name: n2, fields: f2 }) => {
            n1 == n2 && f1.len() == f2.len() && f1.iter().zip(f2).all(|(x, y)| values_equal(x, y))
        }
        // R-6: class equality is IDENTITY — two distinct objects with equal
        // fields are never `equal to`. Comparison is pointer equality of the
        // shared field storage.
        (Value::Object { fields: a, .. }, Value::Object { fields: b, .. }) => std::rc::Rc::ptr_eq(a, b),
        (Value::Closure { function: f1, .. }, Value::Closure { function: f2, .. }) => f1 == f2,
        _ => false,
    }
}

fn cmp_text(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "plus",
        BinOp::Sub => "minus",
        BinOp::Mul => "times",
        BinOp::Div => "divided by",
        BinOp::DivEvenly => "divided evenly by",
        BinOp::Rem => "remainder of",
        BinOp::And => "and",
        BinOp::Or => "or",
        BinOp::Equal => "is equal to",
        BinOp::NotEqual => "is not equal to",
        BinOp::Greater => "is greater than",
        BinOp::Less => "is less than",
        BinOp::AtLeast => "is at least",
        BinOp::AtMost => "is at most",
        BinOp::Contains => "contains",
        BinOp::Max => "bigger of",
        BinOp::Min => "smaller of",
    }
}

/// The conversions (D-39/S-7): `number from` and `decimal from` can fail;
/// `text from` formats with S-9's one-place rules.
fn convert(conv: Conv, v: Value) -> ConvertOutcome {
    match conv {
        Conv::ToNumber => match v {
            Value::Text(s) => match s.trim().parse::<i64>() {
                Ok(n) => ConvertOutcome::Value(Value::Number(n)),
                Err(_) => ConvertOutcome::Fail(format!(
                    "\"{s}\" is not a number — a number is digits, maybe starting with a minus."
                )),
            },
            other => {
                ConvertOutcome::Fail(format!("`number from` needs text, but got {}.", other.format()))
            }
        },
        Conv::ToDecimal => match v {
            Value::Text(s) => match s.trim().parse::<f64>() {
                Ok(d) if d.is_finite() => ConvertOutcome::Value(Value::Decimal(d)),
                _ => ConvertOutcome::Fail(format!("\"{s}\" is not a decimal.")),
            },
            other => ConvertOutcome::Fail(format!(
                "`decimal from` needs text, but got {}.",
                other.format()
            )),
        },
        Conv::ToText => ConvertOutcome::Value(Value::Text(v.format())),
    }
}

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

trait PopFrontLine {
    fn pop_front_line(&mut self) -> Option<String>;
}

impl PopFrontLine for Vec<String> {
    fn pop_front_line(&mut self) -> Option<String> {
        if self.is_empty() {
            None
        } else {
            Some(self.remove(0))
        }
    }
}

// ---------------------------------------------------------------------------
// Tests — M0-faithful execution of the whole MIR instruction set, driven
// end-to-end from Lagom source through the frozen pipeline (parse → check →
// HIR → MIR), so every behavior is pinned exactly as a student writes it.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use lagom_hir as hir;
    use lagom_parser::parse;
    use lagom_sema::check;

    /// Source → MIR through the whole frozen pipeline.
    fn build(src: &str) -> MirProgram {
        let (ast, pdiags) = parse(src);
        assert!(pdiags.is_empty(), "parse errors in test source: {pdiags:?}");
        let checked = check(&ast, src);
        assert!(
            checked.diags.items.is_empty(),
            "sema errors in test source {src:?}: {:?}",
            checked
                .diags
                .items
                .iter()
                .map(|d| (&d.code, &d.message))
                .collect::<Vec<_>>()
        );
        let m = lagom_mir::lower(hir::lower(checked));
        lagom_mir::verify(&m).expect("MIR must verify");
        m
    }

    /// A dev interpreter with the given stdin preloaded, run to completion.
    fn run_dev(src: &str, stdin: &[&str]) -> (Host, RunOutcome) {
        let mut prog = build(src);
        let mut it = Interp::dev(&mut prog);
        it.host.stdin = stdin.iter().map(|s| s.to_string()).collect();
        let outcome = it.run(&prog, src);
        (it.host, outcome)
    }

    fn expect_says(src: &str, stdin: &[&str], want: &[&str]) {
        let (host, outcome) = run_dev(src, stdin);
        assert_eq!(outcome, RunOutcome::Completed, "program failed");
        let want: Vec<String> = want.iter().map(|s| s.to_string()).collect();
        assert_eq!(host.stdout, want);
    }

    // ----- hello world, end to end -----

    #[test]
    fn hello_world() {
        expect_says("say \"Hello, world!\"", &[], &["Hello, world!"]);
    }

    // ----- arithmetic, comparison, boolean operators (S-1) -----

    #[test]
    fn arithmetic_and_precedence() {
        expect_says("say 2 plus 3 times 4", &[], &["14"]);
        expect_says("say 10 minus 3 minus 2", &[], &["5"]);
        expect_says("say 17 divided by 4", &[], &["4.25"]);
        expect_says("say 17 divided evenly by 4", &[], &["4"]);
        expect_says("say 17 remainder of 5", &[], &["2"]);
        expect_says("say 2.5 plus 1.5", &[], &["4"]);
        expect_says("say 1.5 plus 1", &[], &["2.5"]);
    }

    #[test]
    fn division_by_zero_is_a_teaching_trap() {
        let (_, outcome) = run_dev("say 1 divided by 0", &[]);
        match outcome {
            RunOutcome::Panicked { message, .. } => {
                assert!(message.contains("divide by zero"), "{message}");
                assert!(message.contains("line 1"), "{message}");
            }
            other => panic!("expected a panic, got {other:?}"),
        }
    }

    #[test]
    fn overflow_is_a_trap_not_a_wraparound() {
        let (_, outcome) = run_dev("say 9223372036854775807 plus 1", &[]);
        assert!(matches!(outcome, RunOutcome::Panicked { .. }));
    }

    #[test]
    fn comparisons_and_booleans() {
        // R-4: call arguments bind at the additive level — comparisons and
        // boolean operators inside `say` need parentheses.
        expect_says("say (3 is greater than 2)", &[], &["true"]);
        expect_says("say (3 is at least 3)", &[], &["true"]);
        expect_says("say (\"ab\" is equal to \"ab\")", &[], &["true"]);
        expect_says("say (3 is less than 9)", &[], &["true"]);
        expect_says("say (true and false)", &[], &["false"]);
        expect_says("say (true or false)", &[], &["true"]);
        expect_says("say (not false)", &[], &["true"]);
        expect_says("say (2 is equal to 2.0)", &[], &["true"]);
    }

    // ----- say/ask, text, interpolation (7.1, 7.7, S-9) -----

    #[test]
    fn ask_reads_preloaded_lines_in_order() {
        expect_says(
            "make answer equal to ask \"your name?\"\nsay answer",
            &["bo"],
            &["your name?", "bo"],
        );
        expect_says(
            "make x equal to ask \"a?\"\nmake y equal to ask \"b?\"\nsay y\nsay x",
            &["first", "second"],
            &["a?", "b?", "second", "first"],
        );
    }

    #[test]
    fn interpolation_formats_like_say() {
        expect_says("make n equal to 3\nsay \"n is {n}\"", &[], &["n is 3"]);
        expect_says("say \"{1 plus 2}!\"", &[], &["3!"]);
    }

    #[test]
    fn conversions_and_deterministic_text_ops() {
        expect_says(
            "attempt number from \"42\" if it fails then\n    say \"bad\"\notherwise\n    say result",
            &[],
            &["42"],
        );
        expect_says(
            "attempt number from \"x9\" if it fails then\n    say problem\notherwise\n    say \"no\"",
            &[],
            &["\"x9\" is not a number \u{2014} a number is digits, maybe starting with a minus."],
        );
        expect_says("say uppercase of \"lagom\"", &[], &["LAGOM"]);
        expect_says("say size of \"héllo\"", &[], &["5"]);
        expect_says("say text from 12", &[], &["12"]);
    }

    // ----- bindings and mutability (7.2, 8.1) -----

    #[test]
    fn immutable_then_mutable_bindings() {
        expect_says(
            "make x equal to 5\nmake changing y equal to 1\nset y to 10\nincrease y by 5\ndecrease y by 2\nsay y\nsay x",
            &[],
            &["13", "5"],
        );
    }

    #[test]
    fn numeric_bindings_promote_like_expressions() {
        expect_says("make d equal to 1.5 plus 1\nsay d", &[], &["2.5"]);
        expect_says(
            "make changing total equal to 0\nincrease total by 2.5\nsay total",
            &[],
            &["2.5"],
        );
    }

    // ----- control flow: if and all three loop forms (7.4, 7.5) -----

    #[test]
    fn if_else_and_else_if() {
        expect_says(
            "make n equal to 5\nif n is greater than 10\n    say \"big\"\notherwise\n    say \"small\"",
            &[],
            &["small"],
        );
        expect_says(
            "make n equal to 5\nif n is greater than 10\n    say \"big\"\notherwise if n is greater than 3\n    say \"mid\"\notherwise\n    say \"small\"",
            &[],
            &["mid"],
        );
    }

    #[test]
    fn count_loop_with_using_binding() {
        expect_says("repeat 3 times using i\n    say i", &[], &["0", "1", "2"]);
    }

    #[test]
    fn while_loop_counts_down() {
        expect_says(
            "make changing n equal to 3\nrepeat while n is greater than 0\n    say n\n    decrease n by 1",
            &[],
            &["3", "2", "1"],
        );
    }

    #[test]
    fn for_each_walks_the_list() {
        expect_says(
            "make things equal to a list of 10, 20, 30\nrepeat for each t in things\n    say t",
            &[],
            &["10", "20", "30"],
        );
    }

    #[test]
    fn stop_and_next() {
        expect_says(
            "repeat 10 times using i\n    if i is equal to 2\n        stop\n    say i",
            &[],
            &["0", "1"],
        );
        expect_says(
            "repeat 4 times using i\n    if i is equal to 1\n        next\n    say i",
            &[],
            &["0", "2", "3"],
        );
    }

    // ----- lists, maps, pairs, structs (7.6, S-13) -----

    #[test]
    fn list_index_reads_and_writes() {
        expect_says(
            "make changing things equal to a list of 1, 2, 3\nsay things at 1\nset things at 1 to 99\nsay things at 1",
            &[],
            &["2", "99"],
        );
        expect_says(
            "make things equal to a list of 1, 2, 3\nsay first of things\nsay size of things",
            &[],
            &["1", "3"],
        );
    }

    #[test]
    fn index_out_of_range_is_a_failure_inside_attempt() {
        expect_says(
            "make things equal to a list of 1, 2\nattempt say things at 9 if it fails then\n    say \"caught\"\notherwise\n    say \"no\"",
            &[],
            &["caught"],
        );
    }

    #[test]
    fn map_and_pair_and_struct() {
        expect_says(
            "make ages equal to a map from \"ana\" to 11, \"bo\" to 12\nsay ages at \"bo\"",
            &[],
            &["12"],
        );
        expect_says(
            // S-13: `first of` is the list accessor at M0; a pair prints
            // itself (S-9).
            "make pt equal to a pair of 3 and 4\nsay pt",
            &[],
            &["(3, 4)"],
        );
        expect_says(
            "structure player\n    has name of type text\n    has score of type number\n\nmake changing p equal to a player with name \"bo\" and score 7\nsay name of p\nset score of p to 50\nsay score of p",
            &[],
            &["bo", "50"],
        );
    }

    // ----- functions: calls, recursion, returns (7.8) -----

    #[test]
    fn function_calls_and_recursion() {
        expect_says(
            "function square\n    takes number called n\n    gives back n times n\nsay square of 6",
            &[],
            &["36"],
        );
        expect_says(
            "function fact\n    takes number called n\n    if n is at most 1\n        gives back 1\n    gives back n times fact of n minus 1\nsay fact of 5",
            &[],
            &["120"],
        );
        expect_says(
            // R-4: call arguments bind at the additive level, so two chained
            // calls in one `gives back` need parentheses (the frozen rule).
            "function fib\n    takes number called n\n    if n is less than 2\n        gives back n\n    gives back (fib of n minus 1) plus (fib of n minus 2)\nsay fib of 10",
            &[],
            &["55"],
        );
    }

    // ----- the error model: can fail / attempt / fail with (13.1, S-10) -----

    #[test]
    fn fail_with_flows_to_the_innermost_pad() {
        expect_says(
            "function risky business\n    can fail\n    fail with \"boom\"\nfunction wrapper\n    attempt risky business if it fails then\n        say problem\n    otherwise\n        say \"no\"\nwrapper",
            &[],
            &["boom"],
        );
    }

    #[test]
    fn failure_propagates_through_callers_to_the_handler() {
        expect_says(
            "function inner step\n    can fail\n    fail with \"deep\"\nfunction middle step\n    can fail\n    attempt inner step and pass the problem on\nfunction outer step\n    attempt middle step if it fails then\n        say problem\n    otherwise\n        say \"no\"\nouter step",
            &[],
            &["deep"],
        );
    }

    #[test]
    fn as_tail_binds_the_chosen_name() {
        expect_says(
            "function risky business\n    can fail\n    fail with \"boom\"\nattempt risky business as problem\n    say problem\notherwise\n    say result",
            &[],
            &["boom"],
        );
    }

    #[test]
    fn success_branch_binds_result() {
        expect_says(
            "function divide\n    takes number called top\n    takes number called bottom\n    returns a decimal\n    can fail\n    gives back top divided by bottom\nattempt divide 10 and 4 if it fails then\n    say \"no\"\notherwise\n    say result",
            &[],
            &["2.5"],
        );
    }

    #[test]
    fn check_that_passes_and_fails() {
        expect_says("check that 2 plus 2 is equal to 4", &[], &[]);
        let (_, outcome) = run_dev("check that 1 is equal to 2", &[]);
        match outcome {
            RunOutcome::Panicked { message, .. } => {
                assert!(message.contains("check failed"), "{message}");
                assert!(message.contains("left = 1"), "{message}");
                assert!(message.contains("right = 2"), "{message}");
            }
            other => panic!("expected a teaching panic, got {other:?}"),
        }
    }

    // ----- random (S-7): deterministic under a fixed seed -----...

    #[test]
    fn random_is_deterministic_and_in_range() {
        let src = "make r equal to random from 1 to 6\nsay r";
        let mut prog1 = build(src);
        let mut it1 = Interp::dev(&mut prog1);
        it1.host = Host::with_seed(Vec::new(), 42);
        it1.run(&prog1, src);
        let mut prog2 = build(src);
        let mut it2 = Interp::dev(&mut prog2);
        it2.host = Host::with_seed(Vec::new(), 42);
        it2.run(&prog2, src);
        assert_eq!(it1.host.stdout, it2.host.stdout, "same seed, same roll");
        let roll: i64 = it1.host.stdout[0].parse().unwrap();
        assert!((1..=6).contains(&roll), "roll {roll} out of range");
    }

    #[test]
    fn random_seed_varies_replays() {
        let src = "make r equal to random from 1 to 1000000\nsay r";
        let mut prog1 = build(src);
        let mut it1 = Interp::dev(&mut prog1);
        it1.host = Host::with_seed(Vec::new(), 42);
        it1.run(&prog1, src);
        let mut prog2 = build(src);
        let mut it2 = Interp::dev(&mut prog2);
        it2.host = Host::with_seed(Vec::new(), 44);
        it2.run(&prog2, src);
        assert_ne!(it1.host.stdout, it2.host.stdout, "different seeds, different rolls");
    }

    // ----- dev vs release: the observability contract (§26.5) -----

    #[test]
    fn release_builds_record_nothing_and_stay_silent_on_success() {
        let mut prog = build("make x equal to 5\nsay x");
        let mut it = Interp::release(&mut prog);
        it.run(&prog, "make x equal to 5\nsay x");
        assert_eq!(it.host.stdout, vec!["5"]);
        assert!(it.ring().is_none(), "release has no ring");
    }

    #[test]
    fn dev_builds_record_bindings() {
        let mut prog = build("make x equal to 5\nsay x");
        let mut it = Interp::dev(&mut prog);
        it.run(&prog, "make x equal to 5\nsay x");
        assert!(it.ring().expect("dev has a ring").len() >= 1);
    }

    #[test]
    fn passing_dev_builds_explain_nothing_extra() {
        // doc 09: the payoff is on failure — a *passing* run leaves the
        // report out of the output.
        let (host, outcome) = run_dev("say \"fine\"", &[]);
        assert_eq!(outcome, RunOutcome::Completed);
        assert!(
            !host.stdout.iter().any(|l| l.contains("what happened")),
            "a passing dev run must not print a report"
        );
    }

    // ----- the LOM acceptance test (doc 13: seed a failure, the report
    // names the failing value's origin) -----

    /// Builds a program whose script body ends in an unhandled failure —
    /// the same MIR shape a seeded can-fail call produces at the end of the
    /// script, without needing M1 machinery to make one statically. The
    /// script's own bindings still execute, so the report has provenance.
    fn seeded_main(src: &str) -> MirProgram {
        use lagom_mir::{Block, Local, LocalId, Operand, Term};
        let mut prog = build(src);
        let main = prog
            .items
            .iter_mut()
            .find_map(|i| match i {
                MirItem::Main(f) if f.is_entry => Some(f),
                _ => None,
            })
            .expect("script body");
        // The failure machinery: a local carrying the message and a terminal
        // block that fails with it.
        let msg = LocalId(main.locals.len());
        main.locals.push(Local {
            name: "msg".to_string(),
            ty: lagom_sema::Type::Text,
            mutable: false,
            span: Span::new(0, 0),
        });
        let fail_b = BlockId(main.blocks.len());
        main.blocks.push(Block {
            id: fail_b,
            label: "seeded failure".to_string(),
            instrs: vec![Instr::Copy {
                dest: msg,
                value: Operand::Text("secret".to_string()),
                span: Span::new(0, 0),
            }],
            term: Term::Fail {
                message: Operand::Local(msg),
                catch: None,
                span: Span::new(0, 0),
            },
            pad_on_fail: None,
        });
        // Redirect the script's natural end (the implicit `Return`) into the
        // failure block.
        let n_blocks = main.blocks.len();
        for b in main.blocks.iter_mut().take(n_blocks - 1) {
            if matches!(b.term, Term::Return { value: Operand::Nothing }) {
                b.term = Term::Goto(fail_b);
            }
        }
        prog
    }

    #[test]
    fn lom_acceptance_report_names_the_failing_value() {
        let mut prog = seeded_main("make guess equal to 42\nsay guess");
        let mut it = Interp::dev(&mut prog);
        it.host.stdin = Vec::new();
        let outcome = it.run(&prog, "make guess equal to 42\nsay guess");
        let RunOutcome::Failed { message, report } = outcome else {
            panic!("expected a failure, got {outcome:?}");
        };
        assert_eq!(message, "secret");
        let report = report.expect("dev builds render the report");
        assert!(report.contains("what happened"), "{report}");
        assert!(report.contains("`guess` became 42"), "{report}");
        assert!(report.contains("line 1"), "{report}");
        assert!(report.contains("it failed with: secret"), "{report}");
    }

    #[test]
    fn lom_release_build_stays_silent_even_on_failure() {
        let mut prog = seeded_main("make guess equal to 42\nsay guess");
        let mut it = Interp::release(&mut prog);
        let outcome = it.run(&prog, "make guess equal to 42\nsay guess");
        match outcome {
            RunOutcome::Failed { report, .. } => {
                assert!(report.is_none(), "release has no report");
            }
            other => panic!("expected a failure, got {other:?}"),
        }
    }
}
