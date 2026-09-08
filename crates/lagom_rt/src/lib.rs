//! The Lagom runtime (M0 surface) — linked into every native executable.
//!
//! Spec anchors: 21.1 (the `lagom_rt` row: "format, text/list boxing, i64
//! checked arithmetic traps, LOM ring shim"), 8.2 (single data language),
//! 13.1 (errors are values; crashes abort), S-9 (one formatting place shared
//! by `say`/interpolation/`text from`), D-10/D-30 (numeric rules, structural
//! equality), §9 (value semantics), §26.5 (the LOM ring shim + release
//! identity).
//!
//! # ABI contract with generated code (the whole design in one place)
//!
//! Generated code never passes Rust values across the boundary — every value
//! is **two i64s: `(tag, payload)`**, so every signature is plain integers
//! and pointers on every platform (no struct-return ABI questions).
//!
//! - `tag`: the `Value::TAG_*` discriminants below.
//! - `payload`: number bits (i64), decimal *bits* (`f64::to_bits`), boolean
//!   0/1, or a pointer to a heap box (`*mut Value`) for text/list/map/pair/
//!   struct. Boxes are created and owned by the runtime; the M0 bootstrap
//!   leaks them (ARC is the M1 milestone, docs/05 — logged in the M0 report).
//!
//! - **Producing** operations: `rt_x(out: *mut i64, …) -> i64` — `out` points
//!   at a 16-byte caller stack area receiving `(tag, payload)`; the return is
//!   a status: `0` ok, `1` failed (the message is in the fail slot). Panics
//!   exit the process inside rt and never return.
//! - **Void** operations (say, field set, LOM probes) return nothing.
//! - **User function calls** (Cranelift → Cranelift) return three i64s
//!   `(tag, payload, failed)`; on `failed = 1` the message is in the fail
//!   slot. This convention never crosses into Rust.
//!
//! ## The error model (13.1)
//!
//! The **fail slot** is a thread-local holding the in-flight failure message.
//! A failing instruction or `fail with` stores it; the innermost attempt's
//! landing-pad code reads (and consumes) it with `rt_take_fail`. Propagation
//! is "return `failed = 1` without touching the slot", so the message rides
//! up the frame chain until a pad or the program boundary consumes it. At
//! most one failure is in flight at a time (execution is sequential), so the
//! slot needs no queueing.
//!
//! ## The LOM shim (§26.5)
//!
//! Dev builds emit probe calls (`rt_lom_*`) into a bounded
//! [`lagom_mir::EventRing`]; release builds emit none, so the ring stays
//! empty and the unhandled-failure output reduces to the bare failure line —
//! the release-identity invariant holds by construction. The failure report
//! is *the same renderer the interpreter uses* (`lagom_mir::failure_report`),
//! fed by `rt_lom_set_source` at program start.
//!
//! Panics print `panicked on line {line}: {message}` and exit 101 — exactly
//! the interpreter's `Trap::render` format (differential parity is a test).

use lagom_mir::{failure_report, BinOp as MirBinOp, EventRing, LomEvent};
use std::cell::RefCell;
use std::io::Write as _;
use std::rc::Rc;

// ---------------------------------------------------------------------------
// The boxed value (8.2's single data language)
// ---------------------------------------------------------------------------

pub type ListRef = Rc<Vec<Value>>;
pub type MapRef = Rc<Vec<(Value, Value)>>;
pub type StructRef = Rc<(String, Vec<Value>)>;

/// The runtime value. Machine types stay unboxed; the general data language
/// is boxed (§8.2). Text shares its `Rc<String>` on copy — text is immutable
/// at M0, so sharing *is* value semantics; mutable containers deep-copy.
#[derive(Clone, Debug)]
pub enum Value {
    /// D-10: `number` is i64.
    Number(i64),
    /// D-10: `decimal` is f64.
    Decimal(f64),
    Text(Rc<String>),
    Boolean(bool),
    /// The option sentinel (8.5, D-34).
    Nothing,
    List(ListRef),
    Map(MapRef),
    Pair(Box<Value>, Box<Value>),
    Struct(StructRef),
}

impl Value {
    pub const TAG_NUMBER: i64 = 0;
    pub const TAG_DECIMAL: i64 = 1;
    pub const TAG_TEXT: i64 = 2;
    pub const TAG_BOOLEAN: i64 = 3;
    pub const TAG_NOTHING: i64 = 4;
    pub const TAG_LIST: i64 = 5;
    pub const TAG_MAP: i64 = 6;
    pub const TAG_PAIR: i64 = 7;
    pub const TAG_STRUCT: i64 = 8;
    /// The fail payload of a user-call return (`failed = 1`).
    pub const TAG_FAILED: i64 = 9;

    pub fn tag(&self) -> i64 {
        match self {
            Value::Number(_) => Self::TAG_NUMBER,
            Value::Decimal(_) => Self::TAG_DECIMAL,
            Value::Text(_) => Self::TAG_TEXT,
            Value::Boolean(_) => Self::TAG_BOOLEAN,
            Value::Nothing => Self::TAG_NOTHING,
            Value::List(_) => Self::TAG_LIST,
            Value::Map(_) => Self::TAG_MAP,
            Value::Pair(..) => Self::TAG_PAIR,
            Value::Struct(_) => Self::TAG_STRUCT,
        }
    }
}

/// Box a value: its payload is a stable heap pointer (leaked — M0 bootstrap).
fn box_value(v: Value) -> i64 {
    Box::into_raw(Box::new(v)) as i64
}

/// # Safety
/// `pay` must be a pointer returned by [`box_value`] (a live heap box).
unsafe fn deref_value(pay: i64) -> &'static Value {
    &*(pay as *const Value)
}

/// Read a (tag, payload) pair back into a `Value`. Numbers/decimals/bools
/// ride in the payload directly.
unsafe fn unbox(tag: i64, pay: i64) -> Value {
    match tag {
        Value::TAG_NUMBER => Value::Number(pay),
        Value::TAG_DECIMAL => Value::Decimal(f64::from_bits(pay as u64)),
        Value::TAG_TEXT => deref_value(pay).clone(),
        Value::TAG_BOOLEAN => Value::Boolean(pay != 0),
        Value::TAG_NOTHING => Value::Nothing,
        Value::TAG_LIST => deref_value(pay).clone(),
        Value::TAG_MAP => deref_value(pay).clone(),
        Value::TAG_PAIR => deref_value(pay).clone(),
        Value::TAG_STRUCT => deref_value(pay).clone(),
        _ => rt_panic("internal: a value with an unknown tag reached the runtime."),
    }
}

// ---------------------------------------------------------------------------
// S-9: one formatting place (shared by `say`, interpolation, `text from`)
// ---------------------------------------------------------------------------

/// G-2: decimals print shortest-roundtrip; non-finite results render in
/// words — no silent `NaN` in a student's face (mirrors the interpreter).
fn format_decimal(d: f64) -> String {
    if d.is_nan() {
        return "not a number".to_string();
    }
    if d.is_infinite() {
        return if d > 0.0 { "infinity" } else { "-infinity" }.to_string();
    }
    d.to_string()
}

pub fn format_value(v: &Value) -> String {
    match v {
        Value::Number(n) => n.to_string(),
        Value::Decimal(d) => format_decimal(*d),
        Value::Text(s) => s.as_ref().clone(),
        Value::Boolean(b) => (if *b { "true" } else { "false" }).to_string(),
        Value::Nothing => "nothing".to_string(),
        Value::List(items) => {
            let inner: Vec<String> = items.iter().map(format_value).collect();
            format!("[{}]", inner.join(", "))
        }
        Value::Map(entries) => {
            let inner: Vec<String> = entries
                .iter()
                .map(|(k, v)| format!("{}: {}", format_value(k), format_value(v)))
                .collect();
            format!("{{{}}}", inner.join(", "))
        }
        Value::Pair(a, b) => format!("({}, {})", format_value(a), format_value(b)),
        Value::Struct(s) => {
            let inner: Vec<String> = s.1.iter().map(format_value).collect();
            format!("{}({})", s.0, inner.join(", "))
        }
    }
}

// ---------------------------------------------------------------------------
// Panics — crashes, not errors (13.1): not catchable, distinct messages.
// The line number comes from codegen (precomputed from the source span).
// ---------------------------------------------------------------------------

/// The teaching-panic exit: the interpreter's exact `Trap::render` format.
pub fn rt_panic_at(message: &str, line: i64) -> ! {
    eprintln!("panicked on line {line}: {message}");
    std::process::exit(101);
}

/// Internal invariant failures have no line (the interpreter's default too).
pub fn rt_panic(message: &str) -> ! {
    eprintln!("panicked: {message}");
    std::process::exit(101);
}

// ---------------------------------------------------------------------------
// The failure slot (13.1) — errors are values, delivered via the landing pad
// ---------------------------------------------------------------------------

thread_local! {
    static FAIL_SLOT: RefCell<Option<String>> = const { RefCell::new(None) };
    /// Whether this build records LOM events (dev). Release builds leave it
    /// false — the ring stays empty and the unhandled-failure output is the
    /// bare failure line (§26.5's release identity, by construction).
    static DEV: RefCell<bool> = const { RefCell::new(false) };
}

/// Program init: select the observability mode. Dev generated code calls
/// this with 1 before anything runs; release generated code calls it with 0
/// and never emits probe calls (zero observability code).
#[no_mangle]
pub extern "C" fn rt_set_dev(flag: i64) {
    DEV.with(|d| *d.borrow_mut() = flag != 0);
}

fn dev() -> bool {
    DEV.with(|d| *d.borrow())
}

/// Store a failure message and record the LOM failure event in dev builds.
/// Recording lives here so instruction-level failures and explicit `fail
/// with` each produce exactly one event — the MIR `EventFail` probe is
/// dropped by the backend (the interpreter records from the probe instead;
/// the streams are equivalent, pinned by the differential tests).
fn set_fail_message(m: String) {
    if dev() {
        RING.with(|r| r.borrow_mut().push(LomEvent::Failure { message: m.clone() }));
    }
    FAIL_SLOT.with(|s| *s.borrow_mut() = Some(m));
}

/// `fail with` — the message operand arrives as a value.
#[no_mangle]
pub extern "C" fn rt_set_fail(msg_tag: i64, msg_pay: i64) {
    let m = format_value(&unsafe { unbox(msg_tag, msg_pay) });
    set_fail_message(m);
}

/// The attempt's landing pad: consume the stored failure.
/// # Safety
/// `out` must point at 16 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rt_take_fail(out: *mut i64) -> i64 {
    let msg = FAIL_SLOT.with(|s| s.borrow_mut().take());
    match msg {
        Some(m) => {
            let v = Value::Text(Rc::new(m));
            *out = Value::TAG_TEXT;
            *out.add(1) = box_value(v);
            0
        }
        // Compiler bug: a landing pad read with no failure stored.
        None => rt_panic("internal: a failure handler ran with no failure recorded."),
    }
}

/// An index error (or other instruction failure) with no enclosing attempt:
/// a crash with the *failure's* message (13.1's honesty rule — G-19).
#[no_mangle]
pub extern "C" fn rt_panic_from_fail(line: i64) -> ! {
    let msg = FAIL_SLOT.with(|s| s.borrow_mut().take());
    let m = msg.unwrap_or_else(|| "internal: crash with no failure recorded.".to_string());
    rt_panic_at(&m, line)
}

/// The unhandled-failure exit at the program boundary (S-10: non-zero exit).
/// Dev builds print the LOM report first (the ring is empty in release, so
/// release output is the bare failure line — release identity by structure).
pub fn rt_unhandled_failure() -> i32 {
    let msg = FAIL_SLOT.with(|s| s.borrow_mut().take());
    let m = msg.unwrap_or_else(|| "internal: unhandled failure with no message.".to_string());
    if ring_has_events() {
        let report = failure_report(&borrow_ring(), "main", &borrow_source());
        print!("{report}");
        let _ = std::io::stdout().flush();
    }
    println!("the program failed: {m}");
    let _ = std::io::stdout().flush();
    1
}

// ---------------------------------------------------------------------------
// The host: say/ask via the real process
// ---------------------------------------------------------------------------

/// `say` — flush per line (a student watches output appear).
#[no_mangle]
pub extern "C" fn rt_say(tag: i64, pay: i64) {
    let s = format_value(&unsafe { unbox(tag, pay) });
    let out = std::io::stdout();
    let mut lock = out.lock();
    let _ = writeln!(lock, "{s}");
    let _ = lock.flush();
}

/// `ask` — the prompt prints (7.1's visible question), then one stdin line.
/// # Safety
/// `out` must point at 16 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rt_ask(out: *mut i64, q_tag: i64, q_pay: i64, line: i64) -> i64 {
    {
        let out_io = std::io::stdout();
        let mut lock = out_io.lock();
        let _ = write!(lock, "{}", format_value(&unbox(q_tag, q_pay)));
        let _ = lock.flush();
    }
    let mut text = String::new();
    match std::io::stdin().read_line(&mut text) {
        Ok(0) | Err(_) => rt_panic_at("`ask` reached the end of input.", line),
        Ok(_) => {}
    }
    while text.ends_with('\n') || text.ends_with('\r') {
        text.pop();
    }
    *out = Value::TAG_TEXT;
    *out.add(1) = box_value(Value::Text(Rc::new(text)));
    0
}

// ---------------------------------------------------------------------------
// Value semantics (§9): deep copy for mutable containers
// ---------------------------------------------------------------------------

/// Deep-clone a value (§9's binding copy): scalars copy, text shares its
/// immutable `Rc<String>`, mutable containers copy recursively — no two
/// bindings ever share a mutable box, which is exactly what makes in-place
/// container mutation (`rt_index_set`, `rt_field_set`) value-semantic.
fn deep_clone(v: &Value) -> Value {
    match v {
        Value::Number(_) | Value::Decimal(_) | Value::Boolean(_) | Value::Nothing => v.clone(),
        Value::Text(s) => Value::Text(s.clone()),
        Value::List(items) => Value::List(Rc::new(items.iter().map(deep_clone).collect())),
        Value::Map(entries) => Value::Map(Rc::new(
            entries.iter().map(|(k, val)| (deep_clone(k), deep_clone(val))).collect(),
        )),
        Value::Pair(a, b) => Value::Pair(Box::new(deep_clone(a)), Box::new(deep_clone(b))),
        Value::Struct(s) => {
            Value::Struct(Rc::new((s.0.clone(), s.1.iter().map(deep_clone).collect())))
        }
    }
}

/// The binding-copy operation (`rt_clone`): every binding copy in generated
/// code goes through this, so value semantics hold by construction.
/// # Safety
/// `out` must point at 16 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rt_clone(out: *mut i64, tag: i64, pay: i64) -> i64 {
    let v = deep_clone(&unbox(tag, pay));
    write_out(out, &v);
    0
}

// ---------------------------------------------------------------------------
// Text constants: the only values materialized from the data section
// ---------------------------------------------------------------------------

/// A text immediate: copy the constant bytes into an owned box (payload is
/// always a box pointer for text — never raw data-section addresses).
/// # Safety
/// `out` must point at 16 writable bytes; `ptr/len` a valid UTF-8 range.
#[no_mangle]
pub unsafe extern "C" fn rt_const_text(out: *mut i64, ptr: *const u8, len: i64) -> i64 {
    let bytes = std::slice::from_raw_parts(ptr, len as usize);
    let s = String::from_utf8_lossy(bytes).into_owned();
    *out = Value::TAG_TEXT;
    *out.add(1) = box_value(Value::Text(Rc::new(s)));
    0
}

// ---------------------------------------------------------------------------
// Arithmetic and comparison (S-1/S-3, D-10) — the binop table, mirrored from
// the interpreter exactly (differential parity is a test)
// ---------------------------------------------------------------------------

/// `op` is the `MirBinOp` discriminant; `line` feeds the trap messages.
/// # Safety
/// `out` must point at 16 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rt_binop(
    out: *mut i64,
    op: i64,
    l_tag: i64,
    l_pay: i64,
    r_tag: i64,
    r_pay: i64,
    line: i64,
) -> i64 {
    let Some(mop) = num_to_binop(op) else {
        rt_panic("internal: unknown binary operator.");
    };
    let l = unbox(l_tag, l_pay);
    let r = unbox(r_tag, r_pay);
    match binop(mop, l, r, line) {
        Ok(v) => {
            write_out(out, &v);
            0
        }
        Err(msg) => rt_panic_at(&msg, line),
    }
}

fn write_out(out: *mut i64, v: &Value) {
    unsafe {
        *out = v.tag();
        *out.add(1) = match v {
            Value::Number(n) => *n,
            Value::Decimal(d) => d.to_bits() as i64,
            Value::Boolean(b) => *b as i64,
            Value::Nothing => 0,
            _ => box_value(v.clone()),
        };
    }
}

fn num_to_binop(op: i64) -> Option<MirBinOp> {
    // The discriminant order of `lagom_mir::BinOp` (its derive order).
    const ALL: [MirBinOp; 14] = [
        MirBinOp::Add,
        MirBinOp::Sub,
        MirBinOp::Mul,
        MirBinOp::Div,
        MirBinOp::DivEvenly,
        MirBinOp::Rem,
        MirBinOp::And,
        MirBinOp::Or,
        MirBinOp::Equal,
        MirBinOp::NotEqual,
        MirBinOp::Greater,
        MirBinOp::Less,
        MirBinOp::AtLeast,
        MirBinOp::AtMost,
    ];
    ALL.get(op as usize).copied()
}

/// The binop table (S-1/S-3, D-10): `Div` promotes; `DivEvenly` floors.
/// Integer overflow and division by zero are teaching traps (13.1).
fn binop(op: MirBinOp, l: Value, r: Value, line: i64) -> Result<Value, String> {
    use MirBinOp::*;
    let overflow = || "a number grew past its largest possible value (overflow).".to_string();
    match (op, l, r) {
        (Add, Value::Number(a), Value::Number(b)) => {
            a.checked_add(b).map(Value::Number).ok_or_else(overflow)
        }
        (Add, Value::Decimal(a), Value::Decimal(b)) => Ok(Value::Decimal(a + b)),
        (Add, Value::Number(a), Value::Decimal(b)) => Ok(Value::Decimal(a as f64 + b)),
        (Add, Value::Decimal(a), Value::Number(b)) => Ok(Value::Decimal(a + b as f64)),
        (Add, Value::Text(a), Value::Text(b)) => Ok(Value::Text(Rc::new(a.as_ref().clone() + b.as_ref()))),
        (Sub, Value::Number(a), Value::Number(b)) => {
            a.checked_sub(b).map(Value::Number).ok_or_else(overflow)
        }
        (Sub, Value::Decimal(a), Value::Decimal(b)) => Ok(Value::Decimal(a - b)),
        (Sub, Value::Number(a), Value::Decimal(b)) => Ok(Value::Decimal(a as f64 - b)),
        (Sub, Value::Decimal(a), Value::Number(b)) => Ok(Value::Decimal(a - b as f64)),
        (Mul, Value::Number(a), Value::Number(b)) => {
            a.checked_mul(b).map(Value::Number).ok_or_else(overflow)
        }
        (Mul, Value::Decimal(a), Value::Decimal(b)) => Ok(Value::Decimal(a * b)),
        (Mul, Value::Number(a), Value::Decimal(b)) => Ok(Value::Decimal(a as f64 * b)),
        (Mul, Value::Decimal(a), Value::Number(b)) => Ok(Value::Decimal(a * b as f64)),
        // `divided by` always promotes (D-10). Decimal division by zero
        // follows IEEE infinity; the *integer* rule is the teachable trap.
        (Div, Value::Number(a), Value::Number(b)) => {
            if b == 0 {
                Err("cannot divide by zero.".to_string())
            } else {
                Ok(Value::Decimal(a as f64 / b as f64))
            }
        }
        (Div, Value::Decimal(a), Value::Decimal(b)) => Ok(Value::Decimal(a / b)),
        (Div, Value::Number(a), Value::Decimal(b)) => Ok(Value::Decimal(a as f64 / b)),
        (Div, Value::Decimal(a), Value::Number(b)) => Ok(Value::Decimal(a / b as f64)),
        // `divided evenly by` floors; `remainder of` pairs with it.
        (DivEvenly, Value::Number(a), Value::Number(b)) => {
            if b == 0 {
                Err("cannot divide evenly by zero.".to_string())
            } else {
                Ok(Value::Number(a.div_euclid(b)))
            }
        }
        (DivEvenly, Value::Decimal(a), Value::Decimal(b)) => Ok(Value::Decimal((a / b).floor())),
        (DivEvenly, Value::Number(a), Value::Decimal(b)) => {
            Ok(Value::Decimal((a as f64 / b).floor()))
        }
        (DivEvenly, Value::Decimal(a), Value::Number(b)) => {
            Ok(Value::Decimal((a / b as f64).floor()))
        }
        (Rem, Value::Number(a), Value::Number(b)) => {
            if b == 0 {
                Err("cannot take the remainder of zero.".to_string())
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
        (op, l, r) => match compare(op, &l, &r) {
            Some(b) => Ok(Value::Boolean(b)),
            None => Err(format!(
                "these two values cannot be put in order: {} and {}.",
                format_value(&l),
                format_value(&r)
            )),
        },
    }
    .map_err(|e| {
        let _ = line;
        e
    })
}

/// Comparisons: numbers/decimals mix and promote; text and booleans compare
/// exactly; anything else falls to structural equality for Equal/NotEqual
/// (D-30) and refuses the ordered comparisons.
fn compare(op: MirBinOp, l: &Value, r: &Value) -> Option<bool> {
    let ord = match (l, r) {
        (Value::Number(a), Value::Number(b)) => Some(a.cmp(b)),
        (Value::Number(a), Value::Decimal(b)) => (*a as f64).partial_cmp(b),
        (Value::Decimal(a), Value::Number(b)) => a.partial_cmp(&(*b as f64)),
        (Value::Decimal(a), Value::Decimal(b)) => a.partial_cmp(b),
        (Value::Text(a), Value::Text(b)) => Some(a.as_ref().cmp(b.as_ref())),
        (Value::Boolean(a), Value::Boolean(b)) => Some(a.cmp(b)),
        _ => None,
    };
    match ord {
        Some(ord) => Some(
            match op {
                MirBinOp::Equal => ord == std::cmp::Ordering::Equal,
                MirBinOp::NotEqual => ord != std::cmp::Ordering::Equal,
                MirBinOp::Greater => ord == std::cmp::Ordering::Greater,
                MirBinOp::Less => ord == std::cmp::Ordering::Less,
                MirBinOp::AtLeast => ord != std::cmp::Ordering::Less,
                MirBinOp::AtMost => ord != std::cmp::Ordering::Greater,
                _ => return None,
            },
        ),
        None => match op {
            MirBinOp::Equal => Some(values_equal(l, r)),
            MirBinOp::NotEqual => Some(!values_equal(l, r)),
            _ => None,
        },
    }
}

/// Structural equality (D-30) — must match the interpreter exactly.
pub fn values_equal(a: &Value, b: &Value) -> bool {
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
            xs.len() == ys.len() && xs.iter().zip(ys.iter()).all(|(x, y)| values_equal(x, y))
        }
        (Value::Map(xs), Value::Map(ys)) => {
            xs.len() == ys.len()
                && xs.iter().all(|(k, v)| {
                    ys.iter().any(|(k2, v2)| values_equal(k, k2) && values_equal(v, v2))
                })
        }
        (Value::Pair(a1, b1), Value::Pair(a2, b2)) => values_equal(a1, a2) && values_equal(b1, b2),
        (Value::Struct(s1), Value::Struct(s2)) => {
            s1.0 == s2.0
                && s1.1.len() == s2.1.len()
                && s1.1.iter().zip(s2.1.iter()).all(|(x, y)| values_equal(x, y))
        }
        _ => false,
    }
}

/// `op not value` / `minus value` (numeric negation with overflow traps).
/// # Safety
/// `out` must point at 16 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rt_unop(out: *mut i64, op: i64, v_tag: i64, v_pay: i64, line: i64) -> i64 {
    let v = unbox(v_tag, v_pay);
    let result = match (op, v) {
        (0, Value::Number(n)) => match n.checked_neg() {
            Some(x) => Value::Number(x),
            None => rt_panic_at("a number grew past its largest possible value (overflow).", line),
        },
        (0, Value::Decimal(d)) => Value::Decimal(-d),
        (1, Value::Boolean(b)) => Value::Boolean(!b),
        (_, v) => rt_panic_at(&format!("this operation cannot apply to {}.", format_value(&v)), line),
    };
    write_out(out, &result);
    0
}

// ---------------------------------------------------------------------------
// Conversions (D-39, S-7): `number from`/`decimal from` can fail
// ---------------------------------------------------------------------------

/// `conv`: 0 = to number, 1 = to decimal, 2 = to text. Status 1 = failed
/// (the message is in the fail slot).
/// # Safety
/// `out` must point at 16 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rt_convert(out: *mut i64, conv: i64, v_tag: i64, v_pay: i64) -> i64 {
    let v = unbox(v_tag, v_pay);
    let result = match conv {
        0 => match v {
            Value::Text(s) => match s.trim().parse::<i64>() {
                Ok(n) => Value::Number(n),
                Err(_) => {
                    rt_set_fail_text(format!(
                        "\"{s}\" is not a number — a number is digits, maybe starting with a minus."
                    ));
                    return 1;
                }
            },
            other => {
                rt_set_fail_text(format!(
                    "`number from` needs text, but got {}.",
                    format_value(&other)
                ));
                return 1;
            }
        },
        1 => match v {
            Value::Text(s) => match s.trim().parse::<f64>() {
                Ok(d) if d.is_finite() => Value::Decimal(d),
                _ => {
                    rt_set_fail_text(format!("\"{s}\" is not a decimal."));
                    return 1;
                }
            },
            other => {
                rt_set_fail_text(format!(
                    "`decimal from` needs text, but got {}.",
                    format_value(&other)
                ));
                return 1;
            }
        },
        // `text from <value>` — any M0 value formats (S-9).
        _ => Value::Text(Rc::new(format_value(&v))),
    };
    write_out(out, &result);
    0
}

fn rt_set_fail_text(m: String) {
    set_fail_message(m);
}

// ---------------------------------------------------------------------------
// Structs: registration at program start + field access by name
// ---------------------------------------------------------------------------

thread_local! {
    /// (struct name, field names in declaration order) — registered by the
    /// generated program init.
    static STRUCT_FIELDS: RefCell<Vec<(String, Vec<String>)>> = const { RefCell::new(Vec::new()) };
}

/// Register one structure: field names arrive as one `\n`-joined blob.
/// # Safety
/// `name_ptr/name_len` and `fields_ptr/fields_len` must be valid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn rt_register_struct(
    name_ptr: *const u8,
    name_len: i64,
    fields_ptr: *const u8,
    fields_len: i64,
) {
    let name = read_str(name_ptr, name_len);
    let blob = read_str(fields_ptr, fields_len);
    let fields: Vec<String> = blob.split('\n').filter(|s| !s.is_empty()).map(String::from).collect();
    STRUCT_FIELDS.with(|t| {
        let mut t = t.borrow_mut();
        if !t.iter().any(|(n, _)| *n == name) {
            t.push((name, fields));
        }
    });
}

/// # Safety
/// Range must be valid initialized memory.
unsafe fn read_str(ptr: *const u8, len: i64) -> String {
    let bytes = std::slice::from_raw_parts(ptr, len as usize);
    String::from_utf8_lossy(bytes).into_owned()
}

fn field_index(sname: &str, field: &str, line: i64) -> usize {
    STRUCT_FIELDS.with(|t| {
        t.borrow()
            .iter()
            .find(|(n, _)| n == sname)
            .and_then(|(_, fs)| fs.iter().position(|f| f == field))
            .unwrap_or_else(|| {
                rt_panic_at(&format!("`{sname}` has no field called `{field}`."), line)
            })
    })
}

/// `name of player` — field read (the struct's own box shares its name).
/// # Safety
/// `out` must point at 16 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rt_field_get(
    out: *mut i64,
    b_tag: i64,
    b_pay: i64,
    f_ptr: *const u8,
    f_len: i64,
    line: i64,
) -> i64 {
    let fname = read_str(f_ptr, f_len);
    let result = match unbox(b_tag, b_pay) {
        Value::Struct(s) => {
            let ix = field_index(&s.0, &fname, line);
            // A read hands out an owned value (deep clone — §9).
            deep_clone(&s.1[ix])
        }
        other => rt_panic_at(
            &format!("`of` reads a structure's field, but this is {}.", format_value(&other)),
            line,
        ),
    };
    write_out(out, &result);
    0
}

/// `set score of p to 50` — in-place field write. Boxes are never shared
/// (every binding copy deep-clones via `rt_clone`), so in-place *is* value
/// semantics (§9): the rebuilt box replaces the base local's box directly.
/// # Safety
/// `b_pay` must be a live struct box owned by exactly one slot.
#[no_mangle]
pub unsafe extern "C" fn rt_field_set(
    b_tag: i64,
    b_pay: i64,
    f_ptr: *const u8,
    f_len: i64,
    v_tag: i64,
    v_pay: i64,
    line: i64,
) {
    let fname = read_str(f_ptr, f_len);
    let base = unbox(b_tag, b_pay);
    match base {
        Value::Struct(s) => {
            let ix = field_index(&s.0, &fname, line);
            let mut fields: Vec<Value> = s.1.iter().cloned().collect();
            fields[ix] = unbox(v_tag, v_pay);
            let rebuilt = Value::Struct(Rc::new((s.0.clone(), fields)));
            match &mut *unbox_mut(b_pay) {
                slot @ Value::Struct(_) => *slot = rebuilt,
                _ => rt_panic("internal: field write on a non-struct box."),
            }
        }
        other => rt_panic_at(
            &format!("`of` assigns a structure's field, but this is {}.", format_value(&other)),
            line,
        ),
    }
}

// ---------------------------------------------------------------------------
// Lists, maps, pairs (S-13)
// ---------------------------------------------------------------------------

/// # Safety
/// `out` must point at 16 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rt_list_new(out: *mut i64) -> i64 {
    write_out(out, &Value::List(Rc::new(Vec::new())));
    0
}

/// Push onto a *fresh* list (constructor chaining; the box is unshared).
/// # Safety
/// `l_pay` must be a live list box just created by `rt_list_new`.
#[no_mangle]
pub unsafe extern "C" fn rt_list_push(_l_tag: i64, l_pay: i64, v_tag: i64, v_pay: i64) {
    match &mut *unbox_mut(l_pay) {
        Value::List(items) => {
            if let Some(vec) = Rc::get_mut(items) {
                // Constructor chaining builds a *fresh* box; pushed elements
                // deep-clone into it (§9).
                vec.push(deep_clone(&unbox(v_tag, v_pay)));
            } else {
                rt_panic("internal: `rt_list_push` on a shared list.");
            }
        }
        _ => rt_panic("internal: `rt_list_push` on a non-list."),
    }
}

/// # Safety
/// `out` must point at 16 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rt_map_new(out: *mut i64) -> i64 {
    write_out(out, &Value::Map(Rc::new(Vec::new())));
    0
}

/// Insert into a *fresh* map (constructor chaining).
/// # Safety
/// `m_pay` must be a live map box just created by `rt_map_new`.
#[no_mangle]
pub unsafe extern "C" fn rt_map_push(_m_tag: i64, m_pay: i64, k_tag: i64, k_pay: i64, v_tag: i64, v_pay: i64) {
    match &mut *unbox_mut(m_pay) {
        Value::Map(entries) => {
            if let Some(vec) = Rc::get_mut(entries) {
                let k = deep_clone(&unbox(k_tag, k_pay));
                let v = deep_clone(&unbox(v_tag, v_pay));
                vec.push((k, v));
            } else {
                rt_panic("internal: `rt_map_push` on a shared map.");
            }
        }
        _ => rt_panic("internal: `rt_map_push` on a non-map."),
    }
}

/// `dest = a player with name "bo" and score 7` — struct constructor
/// chaining: `rt_struct_new` (no fields) then one `rt_struct_push` per field.
/// # Safety
/// `out` must point at 16 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rt_struct_new(out: *mut i64, n_ptr: *const u8, n_len: i64) -> i64 {
    let name = read_str(n_ptr, n_len);
    write_out(out, &Value::Struct(Rc::new((name, Vec::new()))));
    0
}

/// Push one field onto a *fresh* struct (constructor chaining; field order
/// is the registration order — the generated code emits fields in order).
/// # Safety
/// `s_pay` must be a live struct box just created by `rt_struct_new`.
#[no_mangle]
pub unsafe extern "C" fn rt_struct_push(s_pay: i64, _f_ptr: *const u8, _f_len: i64, v_tag: i64, v_pay: i64) {
    // Field names are not stored per-value: `field_index` resolves against
    // the registration table at access time, so the push only appends the
    // value in declaration order.
    let v = deep_clone(&unbox(v_tag, v_pay));
    match &mut *unbox_mut(s_pay) {
        Value::Struct(s) => {
            if let Some((_, fields)) = Rc::get_mut(s) {
                fields.push(v);
            } else {
                rt_panic("internal: `rt_struct_push` on a shared struct.");
            }
        }
        _ => rt_panic("internal: `rt_struct_push` on a non-struct."),
    }
}

/// # Safety
/// `out` must point at 16 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rt_pair_new(out: *mut i64, a_tag: i64, a_pay: i64, b_tag: i64, b_pay: i64) -> i64 {
    let a = unbox(a_tag, a_pay);
    let b = unbox(b_tag, b_pay);
    write_out(out, &Value::Pair(Box::new(deep_clone(&a)), Box::new(deep_clone(&b))));
    0
}

/// Index read: lists trap on out-of-range *unless* a failure is wanted — the
/// status protocol covers both: status 1 = the message is in the fail slot
/// (the attempt's pad consumes it; with no pad codegen turns it into a crash
/// via `rt_panic_from_fail`, matching the interpreter's G-19 honesty rule).
/// # Safety
/// `out` must point at 16 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rt_index_get(
    out: *mut i64,
    b_tag: i64,
    b_pay: i64,
    i_tag: i64,
    i_pay: i64,
    line: i64,
) -> i64 {
    let b = unbox(b_tag, b_pay);
    let i = unbox(i_tag, i_pay);
    match (b, i) {
        (Value::List(items), Value::Number(n)) => match index_of(items.len(), n) {
            Ok(ix) => {
                // A read hands out an owned value (deep clone — §9).
                write_out(out, &deep_clone(&items[ix]));
                0
            }
            Err(msg) => {
                rt_set_fail_text(msg);
                let _ = line;
                1
            }
        },
        (Value::Map(entries), key) => {
            // A map read of a missing key gives `nothing` (S-13/D-34).
            let found = entries
                .iter()
                .find(|(k, _)| values_equal(k, &key))
                .map(|(_, v)| v.clone())
                .unwrap_or(Value::Nothing);
            write_out(out, &found);
            0
        }
        (b, i) => {
            rt_set_fail_text(format!(
                "`at` reads a list or a map, but {} at {} is not one.",
                format_value(&b),
                format_value(&i)
            ));
            1
        }
    }
}

/// Index/entry write: in-place on the base local's box (never shared).
/// Status 1 = out-of-range list write (message in the fail slot).
/// # Safety
/// `b_pay` must be a live list/map box owned by exactly one local.
#[no_mangle]
pub unsafe extern "C" fn rt_index_set(
    _b_tag: i64,
    b_pay: i64,
    i_tag: i64,
    i_pay: i64,
    v_tag: i64,
    v_pay: i64,
    _line: i64,
) -> i64 {
    let i = unbox(i_tag, i_pay);
    let v = unbox(v_tag, v_pay);
    match &*deref_value(b_pay) {
        Value::List(items) => {
            let n = match &i {
                Value::Number(n) => *n,
                _ => {
                    rt_set_fail_text(format!(
                        "`at` assigns into a list or a map, but a list at {} is not one.",
                        format_value(&i)
                    ));
                    return 1;
                }
            };
            let ix = match index_of(items.len(), n) {
                Ok(ix) => ix,
                Err(msg) => {
                    rt_set_fail_text(msg);
                    return 1;
                }
            };
            match &mut *unbox_mut(b_pay) {
                Value::List(items) => {
                    if let Some(vec) = Rc::get_mut(items) {
                        vec[ix] = v;
                        0
                    } else {
                        rt_panic("internal: index write on a shared list.");
                    }
                }
                _ => unreachable!("matched above"),
            }
        }
        Value::Map(_entries) => {
            match &mut *unbox_mut(b_pay) {
                Value::Map(ents) => {
                    if let Some(vec) = Rc::get_mut(ents) {
                        match vec.iter_mut().find(|(k, _)| values_equal(k, &i)) {
                            Some(slot) => slot.1 = v,
                            None => vec.push((i, v)),
                        }
                        0
                    } else {
                        rt_panic("internal: index write on a shared map.");
                    }
                }
                _ => unreachable!("matched above"),
            }
        }
        other => {
            rt_set_fail_text(format!(
                "`at` assigns into a list or a map, but this is {}.",
                format_value(other)
            ));
            1
        }
    }
}

/// # Safety
/// `pay` must be a live box pointer.
unsafe fn unbox_mut(pay: i64) -> *mut Value {
    pay as *mut Value
}

fn index_of(len: usize, n: i64) -> Result<usize, String> {
    if n < 0 {
        return Err(format!("a list index cannot be negative, but this is {n}."));
    }
    let n = n as usize;
    if n >= len {
        return Err(format!("index {n} is past the end of this list ({len} items)."));
    }
    Ok(n)
}

/// The `Branch` terminator's condition check: a non-boolean condition is a
/// crash (impossible in checked programs — the verifier's invariant).
/// Returns 1 for true, 0 for false.
/// # Safety
/// Operands must be live values.
#[no_mangle]
pub unsafe extern "C" fn rt_branch_check(c_tag: i64, c_pay: i64) -> i64 {
    match unbox(c_tag, c_pay) {
        Value::Boolean(b) => b as i64,
        other => rt_panic(&format!(
            "a condition must be true or false, but this is {}.",
            format_value(&other)
        )),
    }
}

// ---------------------------------------------------------------------------
// The remaining builtin surface
// ---------------------------------------------------------------------------

/// Interpolation: build a fresh text, then push literal/value parts.
/// # Safety
/// `out` must point at 16 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rt_format_new(out: *mut i64) -> i64 {
    write_out(out, &Value::Text(Rc::new(String::new())));
    0
}

/// Push a literal chunk onto a *fresh* format text.
/// # Safety
/// `t_pay` fresh text box; `ptr/len` valid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn rt_format_push_lit(_t_tag: i64, t_pay: i64, ptr: *const u8, len: i64) {
    let chunk = read_str(ptr, len);
    match &mut *unbox_mut(t_pay) {
        Value::Text(s) => {
            if let Some(st) = Rc::get_mut(s) {
                st.push_str(&chunk);
            } else {
                rt_panic("internal: format push on a shared text.");
            }
        }
        _ => rt_panic("internal: format push on a non-text."),
    }
}

/// Push a formatted value onto a *fresh* format text (S-9's one place).
/// # Safety
/// `t_pay` fresh text box.
#[no_mangle]
pub unsafe extern "C" fn rt_format_push_val(_t_tag: i64, t_pay: i64, v_tag: i64, v_pay: i64) {
    let v = unbox(v_tag, v_pay);
    let rendered = format_value(&v);
    match &mut *unbox_mut(t_pay) {
        Value::Text(s) => {
            if let Some(st) = Rc::get_mut(s) {
                st.push_str(&rendered);
            } else {
                rt_panic("internal: format push on a shared text.");
            }
        }
        _ => rt_panic("internal: format push on a non-text."),
    }
}

/// `random from lo to hi` — deterministic xorshift64* (interp parity).
/// # Safety
/// `out` must point at 16 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rt_random(out: *mut i64, lo_tag: i64, lo_pay: i64, hi_tag: i64, hi_pay: i64, line: i64) -> i64 {
    let lo = unbox(lo_tag, lo_pay);
    let hi = unbox(hi_tag, hi_pay);
    match (lo, hi) {
        (Value::Number(lo), Value::Number(hi)) => {
            if hi < lo {
                rt_panic_at(&format!("`random from {lo} to {hi}` has its bounds reversed."), line);
            }
            let span_len = (hi - lo) as u64 + 1;
            let r = next_random() % span_len;
            write_out(out, &Value::Number(lo + r as i64));
            0
        }
        _ => rt_panic_at("`random from … to …` needs two numbers.", line),
    }
}

thread_local! {
    /// Seed: the clock at startup (runs vary); the driver can pin it.
    static RNG: RefCell<u64> = const { RefCell::new(0x2545_F491_4F6C_DD1D) };
}

fn next_random() -> u64 {
    RNG.with(|r| {
        let mut x = *r.borrow();
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        *r.borrow_mut() = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    })
}

/// Wall-clock seed at program start (runs play differently; `--seed` pins —
/// replay is an interpreter-side M0 capability, native parity excludes it).
pub fn init_random() {
    // Replay determinism (26.5): `LAGOM_SEED=<u64>` pins the sequence, the
    // same contract the interpreter's `Host::with_seed` offers. Unset, the
    // seed comes from the clock — two runs play differently.
    if let Ok(seed) = std::env::var("LAGOM_SEED") {
        if let Ok(n) = seed.parse::<u64>() {
            RNG.with(|r| *r.borrow_mut() = n | 1);
            return;
        }
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64 ^ d.as_secs())
        .unwrap_or(0x2545_F491_4F6C_DD1D);
    RNG.with(|r| *r.borrow_mut() = nanos | 1);
}

/// `first of list` — the option-producing head query (S-13).
/// # Safety
/// `out` must point at 16 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rt_first_of(out: *mut i64, l_tag: i64, l_pay: i64, line: i64) -> i64 {
    let l = unbox(l_tag, l_pay);
    match l {
        Value::List(items) => {
            // A read hands out an owned value (deep clone — §9); an empty
            // list gives `nothing` (S-13's option shape).
            let first = items.first().map(deep_clone).unwrap_or(Value::Nothing);
            write_out(out, &first);
            0
        }
        other => rt_panic_at(
            &format!("`first of` needs a list, but this is {}.", format_value(&other)),
            line,
        ),
    }
}

/// `size of` — lists, maps, text.
/// # Safety
/// `out` must point at 16 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rt_size_of(out: *mut i64, v_tag: i64, v_pay: i64, line: i64) -> i64 {
    let v = unbox(v_tag, v_pay);
    let result = match v {
        Value::List(items) => Value::Number(items.len() as i64),
        Value::Map(entries) => Value::Number(entries.len() as i64),
        Value::Text(s) => Value::Number(s.chars().count() as i64),
        other => rt_panic_at(
            &format!("`size of` needs a list, map, or text, but this is {}.", format_value(&other)),
            line,
        ),
    };
    write_out(out, &result);
    0
}

/// `join list` — concatenated S-9 renderings.
/// # Safety
/// `out` must point at 16 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rt_join(out: *mut i64, l_tag: i64, l_pay: i64, line: i64) -> i64 {
    let l = unbox(l_tag, l_pay);
    match l {
        Value::List(items) => {
            let joined: String = items.iter().map(format_value).collect();
            write_out(out, &Value::Text(Rc::new(joined)));
            0
        }
        other => rt_panic_at(
            &format!("`join` needs a list of text, but this is {}.", format_value(&other)),
            line,
        ),
    }
}

/// Text operations: 0 uppercase, 1 lowercase, 2 trim.
/// # Safety
/// `out` must point at 16 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rt_textop(out: *mut i64, op: i64, v_tag: i64, v_pay: i64, line: i64) -> i64 {
    let v = unbox(v_tag, v_pay);
    let result = match v {
        Value::Text(s) => Value::Text(Rc::new(match op {
            0 => s.to_uppercase(),
            1 => s.to_lowercase(),
            _ => s.trim().to_string(),
        })),
        other => rt_panic_at(
            &format!("this text operation needs text, but got {}.", format_value(&other)),
            line,
        ),
    };
    write_out(out, &result);
    0
}

/// Math operations: 0 square root, 1 floor.
/// # Safety
/// `out` must point at 16 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rt_mathop(out: *mut i64, op: i64, v_tag: i64, v_pay: i64, line: i64) -> i64 {
    let v = unbox(v_tag, v_pay);
    let result = match (op, v) {
        (0, Value::Number(n)) => Value::Decimal((n as f64).sqrt()),
        (0, Value::Decimal(d)) => Value::Decimal(d.sqrt()),
        (1, Value::Decimal(d)) => Value::Number(d.floor() as i64),
        (1, Value::Number(n)) => Value::Number(n),
        (_, other) => rt_panic_at(
            &format!(
                "this math operation needs a number or decimal, but got {}.",
                format_value(&other)
            ),
            line,
        ),
    };
    write_out(out, &result);
    0
}

fn cmp_text(op: MirBinOp) -> &'static str {
    match op {
        MirBinOp::Add => "plus",
        MirBinOp::Sub => "minus",
        MirBinOp::Mul => "times",
        MirBinOp::Div => "divided by",
        MirBinOp::DivEvenly => "divided evenly by",
        MirBinOp::Rem => "remainder of",
        MirBinOp::And => "and",
        MirBinOp::Or => "or",
        MirBinOp::Equal => "is equal to",
        MirBinOp::NotEqual => "is not equal to",
        MirBinOp::Greater => "is greater than",
        MirBinOp::Less => "is less than",
        MirBinOp::AtLeast => "is at least",
        MirBinOp::AtMost => "is at most",
    }
}

/// `check that value` — G-4: a failed check renders the comparison, naming
/// both sides, then the crash (a panic, not an error value).
/// # Safety
/// All payloads must be live values.
#[no_mangle]
pub unsafe extern "C" fn rt_check(
    v_tag: i64,
    v_pay: i64,
    l_tag: i64,
    l_pay: i64,
    r_tag: i64,
    r_pay: i64,
    cmp_op: i64,
    has_cmp: i64,
    line: i64,
) {
    let v = unbox(v_tag, v_pay);
    if matches!(v, Value::Boolean(true)) {
        return;
    }
    let detail = if has_cmp != 0 {
        let l = unbox(l_tag, l_pay);
        let r = unbox(r_tag, r_pay);
        let op = num_to_binop(cmp_op).map(cmp_text).unwrap_or("?");
        format!("check failed: {op} (left = {}, right = {})", format_value(&l), format_value(&r))
    } else {
        "check failed".to_string()
    };
    rt_panic_at(&detail, line);
}

// ---------------------------------------------------------------------------
// The LOM shim (§26.5): dev builds record into the shared ring; release
// builds make no calls at all, so the ring stays empty and output is bare.
// ---------------------------------------------------------------------------

thread_local! {
    static RING: RefCell<EventRing> = RefCell::new(EventRing::new(256));
    /// The program source, for line numbers in the report (set at init).
    static SOURCE: RefCell<String> = RefCell::new(String::new());
    /// Scratch accumulating the current entry event's args.
    static ENTRY_ARGS: RefCell<Vec<(String, String)>> = RefCell::new(Vec::new());
    static ENTRY_FN: RefCell<String> = RefCell::new(String::new());
}

fn ring_has_events() -> bool {
    RING.with(|r| !r.borrow().is_empty())
}

fn borrow_ring() -> EventRing {
    // The renderer takes ownership-shaped access; clone the (small, bounded)
    // ring out — the report is rendered once per process.
    RING.with(|r| {
        let ring = r.borrow();
        clone_ring(&ring)
    })
}

fn clone_ring(ring: &EventRing) -> EventRing {
    let mut out = EventRing::new(256);
    for e in ring.events() {
        out.push(e.clone());
    }
    out
}

fn borrow_source() -> String {
    SOURCE.with(|s| s.borrow().clone())
}

/// Program init: hand the source text to the report renderer.
/// # Safety
/// Range must be valid initialized memory.
#[no_mangle]
pub unsafe extern "C" fn rt_lom_set_source(ptr: *const u8, len: i64) {
    SOURCE.with(|s| *s.borrow_mut() = read_str(ptr, len));
}

/// Begin a function-entry event.
/// # Safety
/// Range must be valid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn rt_lom_entry(f_ptr: *const u8, f_len: i64) {
    let name = read_str(f_ptr, f_len);
    ENTRY_FN.with(|s| *s.borrow_mut() = name);
    ENTRY_ARGS.with(|a| a.borrow_mut().clear());
}

/// One argument of the current entry event.
/// # Safety
/// Range must be valid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn rt_lom_entry_arg(n_ptr: *const u8, n_len: i64, v_tag: i64, v_pay: i64) {
    let name = read_str(n_ptr, n_len);
    let v = unbox(v_tag, v_pay);
    ENTRY_ARGS.with(|a| a.borrow_mut().push((name, format_value(&v))));
}

/// Flush the current entry event into the ring (called after the last arg).
#[no_mangle]
pub extern "C" fn rt_lom_entry_end() {
    let args = ENTRY_ARGS.with(|a| std::mem::take(&mut *a.borrow_mut()));
    let function = ENTRY_FN.with(|s| s.borrow().clone());
    RING.with(|r| {
        r.borrow_mut()
            .push(LomEvent::FunctionEntry { function, args })
    });
}

/// A binding record (26.5's value provenance).
/// # Safety
/// Range must be valid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn rt_lom_bind(n_ptr: *const u8, n_len: i64, v_tag: i64, v_pay: i64, site: i64) {
    let name = read_str(n_ptr, n_len);
    let v = unbox(v_tag, v_pay);
    RING.with(|r| {
        r.borrow_mut().push(LomEvent::Bind {
            name,
            value: format_value(&v),
            site: site as usize,
        })
    });
}

/// A failure about to propagate (the ring's payoff record).
#[no_mangle]
pub extern "C" fn rt_lom_fail(m_tag: i64, m_pay: i64) {
    let m = unsafe { unbox(m_tag, m_pay) };
    RING.with(|r| {
        r.borrow_mut()
            .push(LomEvent::Failure { message: format_value(&m) })
    });
}

// ---------------------------------------------------------------------------
// The entry shim, linked into every executable (23.5: platform linker —
// the driver drives `rustc`, which drives the platform linker)
// ---------------------------------------------------------------------------

// The generated program entry: `() -> i64` where the return is the failed
// flag (all struct-returning conventions stay Cranelift-internal).
// The generated program entry (crates/lagom_codegen): `() -> i64` where the
// return is the failed flag. Not declared in test builds — the rt unit tests
// exercise the runtime surface itself, not a linked Lagom program.
#[cfg(not(test))]
extern "C" {
    fn lagom_main() -> i64;
}

/// The Rust `main` the driver's shim calls; runs program init then the user
/// script. Returns the process exit code (0 ok, 1 unhandled failure).
#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn __lagom_start() -> i32 {
    init_random();
    let failed = unsafe { lagom_main() };
    if failed == 0 {
        0
    } else {
        rt_unhandled_failure()
    }
}
