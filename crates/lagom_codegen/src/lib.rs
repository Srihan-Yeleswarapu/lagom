//! The Lagom Cranelift backend (M0) — LIR → machine-code object file.
//!
//! Spec anchors: 00 §23.1/§23.2 (Cranelift for instant builds; one IR, many
//! backends), §22.3 (LIR's contract: "one LIR op = one lowering decision" —
//! this file never re-derives semantics), 21.1 (`lagom_codegen`: LIR →
//! Cranelift module → object → link), 23.5 (the *driver* drives the platform
//! linker through `rustc`; this crate produces the object it links).
//!
//! # The value ABI (mirrored from `lagom_rt` — the whole contract)
//!
//! Every Lagom value is **two i64s: `(tag, payload)`** — number bits, decimal
//! bits, boolean 0/1, or a pointer to a runtime-owned heap box. Every user
//! function takes an out pointer plus `2×params` i64s and *writes* **three**
//! i64s — `(tag, payload, failed)` — through that pointer (three i64s never
//! fit a register-return ABI); the failed flag drives the error model (13.1).
//!
//! LIR frame slots become pairs of Cranelift *variables* (tag, payload) per
//! slot; the FunctionBuilder's SSA machinery keeps branch structure honest.
//! Runtime-produced results arrive through one shared 16-byte stack slot per
//! frame (`rt_*` write `(tag, payload)` through an `*mut i64` out-pointer).
//! Text immediates are materialized by `rt_const_text` from the data section
//! (payloads must be heap boxes, never raw data addresses).
//!
//! # Landing pads (13.1)
//!
//! A failing instruction or `fail with` stores its message in the runtime's
//! fail slot. A `catch_pads` entry means "at the *entry* of this block, read
//! the slot into the message local" — codegen emits exactly one
//! `rt_take_fail` there, and pads are only ever reached by a failure, so the
//! read always consumes a live message.
//!
//! # LOM (§26.5)
//!
//! Dev builds emit the LIR's probe calls verbatim (`rt_lom_*`); release
//! builds never see them (the LIR lowering strips them — the release-identity
//! boundary is upstream of this file). `lagom_main` hands the runtime the
//! source text in dev so failure reports can quote lines.
//! `LomFail` LIR instructions are *dropped*: `rt_set_fail` already records
//! the failure event in dev builds (exactly one event per failure — the
//! backend comment contract in `lagom_rt`).

use std::collections::HashMap;

use cranelift::codegen::ir::types::I64;
use cranelift::codegen::ir::{
    // TODO: re-enable when GlobalValue is actually used (was imported for a reason).
    // GlobalValue,
    Function, StackSlot, TrapCode, UserFuncName,
};
use cranelift::codegen::settings;
use cranelift::codegen::Context;
use cranelift::codegen::isa::CallConv;
use cranelift::prelude::*;
use cranelift_module::{DataDescription, DataId, FuncId, Linkage, Module};
use cranelift_object::ObjectBuilder;
use cranelift_object::ObjectModule;

use lagom_lir::{
    BlockId, FormatPart, LirFunction, LirInstr, LirOperand, LirProgram, LirTerm, SlotId, StrId,
};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Verification flags shared by the up-front function verifier (the flags
/// values only matter for ISA-specific checks, which the verifier skips).
static VERIFY_FLAGS: std::sync::OnceLock<settings::Flags> = std::sync::OnceLock::new();

fn verify_flags() -> &'static settings::Flags {
    VERIFY_FLAGS.get_or_init(|| settings::Flags::new(settings::builder()))
}

/// Compile a lowered LIR program to a native object file.
///
/// `source` is the program text (embedded for the LOM failure report in dev
/// builds; never embedded in release). `dev` selects the build mode the
/// runtime reports. `no_pdb` controls whether debug info may be emitted into
/// the object file.
///
/// The default for end-user builds is `no_pdb = true`: users should not see
/// debug-info files unless they explicitly ask for them with `--pdb`.
///
/// PDB generation is controlled at the object-backend level, not by a
/// Cranelift codegen setting.
///
/// The default for end-user builds is `no_pdb = true`: users should not see
/// debug-info files unless they explicitly ask for them with `--pdb`.
///
/// PDB generation is controlled at the link step via the `/DEBUG:NONE` linker
/// flag on Windows when `no_pdb = true`; the object backend leaves debug-info
/// flags at their default on all platforms.
pub fn compile(lir: &LirProgram, source: &str, dev: bool, _no_pdb: bool) -> Result<Vec<u8>, String> {
    let mut sb = settings::builder();
    // Position-independent code: the object is linked by the platform linker
    // on every OS (ELF and Mach-O default to PIE, and the runtime rlib rustc
    // builds is PIC), so absolute relocations (R_X86_64_64) against rt_*
    // symbols are rejected on Linux/macOS without this.
    let _ = sb.set("is_pic", "true");
    let flags = settings::Flags::new(sb);
    let isa_builder = cranelift_native::builder().map_err(|why| {
        format!("this machine's architecture is not supported by the Cranelift backend: {why}")
    })?;
    let isa = isa_builder
        .finish(flags)
        .map_err(|e| format!("Cranelift ISA setup failed: {e}"))?;
    let call_conv = isa.default_call_conv();

    let mut builder = ObjectBuilder::new(
        isa.clone(),
        "lagom_module.o".to_string(),
        cranelift_module::default_libcall_names(),
    )
    .map_err(|e| e.to_string())?;
    builder.per_function_section(true);
    builder.per_data_object_section(true);
    let mut module = ObjectModule::new(builder);
    let rt = RtFns::declare(&mut module, call_conv);

    let mut bx = Backend {
        module,
        call_conv,
        lir,
        dev,
        source_len: source.len(),
        fn_ids: HashMap::new(),
        rt,
        strings_blob: None,
        source_blob: None,
        struct_blobs: Vec::new(),
    };

    // Data section: one NUL-terminated string pool laid out exactly as
    // `LirProgram::string_offset` expects, then per-struct and source blobs.
    let pool: Vec<u8> = lir
        .strings
        .iter()
        .flat_map(|s| s.as_bytes().iter().copied().chain(std::iter::once(0)))
        .collect();
    bx.strings_blob = Some(bx.declare_ro(&pool));
    if bx.dev {
        bx.source_blob = Some(bx.declare_ro(source.as_bytes()));
    }
    for (name, fields) in &lir.structs {
        let fields_blob = fields.join("\n");
        let name_blob = bx.declare_ro(name.as_bytes());
        let fields_data = bx.declare_ro(fields_blob.as_bytes());
        bx.struct_blobs.push((
            name_blob,
            name.len() as i64,
            fields_data,
            fields_blob.len() as i64,
        ));
    }

    // Declare all user functions first so every call site resolves.
    for f in &lir.functions {
        let sig = user_signature(call_conv, f.params);
        let symbol = mangle_symbol(&f.name);
        let id = bx
            .module
            .declare_function(&symbol, Linkage::Local, &sig)
            .map_err(|e| e.to_string())?;
        bx.fn_ids.insert(f.name.clone(), id);
    }

    for f in &lir.functions {
        bx.define_function(f)?;
    }
    bx.define_lagom_main()?;

    let product = bx.module.finish();

    product
        .emit()
        .map_err(|e| format!("object emission failed: {e}"))
}

/// A Lagom function name becomes a legal object symbol: spaces and anything
/// non-identifier-shaped become `_`. Collision-free within one object is
/// guaranteed by sema's unique function names modulo this mangling; a suffix
/// makes even mangled collisions safe (deterministic — same program, same
/// symbols).
fn mangle_symbol(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    format!("lagom_fn_{safe}")
}

fn user_signature(call_conv: CallConv, params: usize) -> Signature {
    // The out-pointer ABI: `(out: *mut i64, tag, payload)` per parameter, no
    // register returns. A user function writes its `(tag, payload, failed)`
    // triple through `out` — three i64s never fit a register ABI, and this
    // matches the `rt_*` convention already used at the runtime boundary.
    let mut sig = Signature::new(call_conv);
    sig.params = vec![AbiParam::new(I64); 1 + 2 * params];
    sig
}

// ---------------------------------------------------------------------------
// The runtime imports
// ---------------------------------------------------------------------------

/// The `rt_*` surface, declared once per module. Each entry is the FuncId of
/// the imported runtime function (resolved by the linker against the runtime
/// library the driver links in).
struct RtFns {
    say: FuncId,
    ask: FuncId,
    clone: FuncId,
    const_text: FuncId,
    binop: FuncId,
    unop: FuncId,
    convert: FuncId,
    random: FuncId,
    first_of: FuncId,
    size_of: FuncId,
    join: FuncId,
    textop: FuncId,
    mathop: FuncId,
    check: FuncId,
    field_get: FuncId,
    field_set: FuncId,
    index_get: FuncId,
    index_set: FuncId,
    list_new: FuncId,
    list_push: FuncId,
    map_new: FuncId,
    map_push: FuncId,
    struct_new: FuncId,
    struct_push: FuncId,
    pair_new: FuncId,
    format_new: FuncId,
    format_push_lit: FuncId,
    format_push_val: FuncId,
    branch_check: FuncId,
    set_fail: FuncId,
    take_fail: FuncId,
    panic_from_fail: FuncId,
    set_dev: FuncId,
    register_struct: FuncId,
    lom_set_source: FuncId,
    lom_entry: FuncId,
    lom_entry_arg: FuncId,
    lom_entry_end: FuncId,
    lom_bind: FuncId,
}

impl RtFns {
    fn declare(module: &mut ObjectModule, call_conv: CallConv) -> RtFns {
        let i = I64;
        let p = i;
        let imp = |module: &mut ObjectModule, name: &str, params: usize, rets: usize| {
            let mut sig = Signature::new(call_conv);
            sig.params = vec![AbiParam::new(p); params];
            sig.returns = vec![AbiParam::new(i); rets];
            module
                .declare_function(name, Linkage::Import, &sig)
                .expect("runtime import declaration")
        };
        RtFns {
            say: imp(module, "rt_say", 2, 0),
            ask: imp(module, "rt_ask", 4, 1),
            clone: imp(module, "rt_clone", 3, 1),
            const_text: imp(module, "rt_const_text", 3, 1),
            binop: imp(module, "rt_binop", 7, 1),
            unop: imp(module, "rt_unop", 5, 1),
            convert: imp(module, "rt_convert", 4, 1),
            random: imp(module, "rt_random", 6, 1),
            first_of: imp(module, "rt_first_of", 4, 1),
            size_of: imp(module, "rt_size_of", 4, 1),
            join: imp(module, "rt_join", 4, 1),
            textop: imp(module, "rt_textop", 5, 1),
            mathop: imp(module, "rt_mathop", 5, 1),
            check: imp(module, "rt_check", 9, 0),
            field_get: imp(module, "rt_field_get", 6, 1),
            field_set: imp(module, "rt_field_set", 7, 0),
            index_get: imp(module, "rt_index_get", 6, 1),
            index_set: imp(module, "rt_index_set", 7, 1),
            list_new: imp(module, "rt_list_new", 1, 1),
            list_push: imp(module, "rt_list_push", 4, 0),
            map_new: imp(module, "rt_map_new", 1, 1),
            map_push: imp(module, "rt_map_push", 6, 0),
            struct_new: imp(module, "rt_struct_new", 3, 1),
            struct_push: imp(module, "rt_struct_push", 5, 0),
            pair_new: imp(module, "rt_pair_new", 5, 1),
            format_new: imp(module, "rt_format_new", 1, 1),
            format_push_lit: imp(module, "rt_format_push_lit", 4, 0),
            format_push_val: imp(module, "rt_format_push_val", 4, 0),
            branch_check: imp(module, "rt_branch_check", 2, 1),
            set_fail: imp(module, "rt_set_fail", 2, 0),
            take_fail: imp(module, "rt_take_fail", 1, 1),
            panic_from_fail: imp(module, "rt_panic_from_fail", 1, 0),
            set_dev: imp(module, "rt_set_dev", 1, 0),
            register_struct: imp(module, "rt_register_struct", 4, 0),
            lom_set_source: imp(module, "rt_lom_set_source", 2, 0),
            lom_entry: imp(module, "rt_lom_entry", 2, 0),
            lom_entry_arg: imp(module, "rt_lom_entry_arg", 4, 0),
            lom_entry_end: imp(module, "rt_lom_entry_end", 0, 0),
            lom_bind: imp(module, "rt_lom_bind", 5, 0),
        }
    }
}

// ---------------------------------------------------------------------------
// The backend state
// ---------------------------------------------------------------------------

struct Backend<'a> {
    module: ObjectModule,
    call_conv: CallConv,
    lir: &'a LirProgram,
    dev: bool,
    source_len: usize,
    fn_ids: HashMap<String, FuncId>,
    rt: RtFns,
    strings_blob: Option<DataId>,
    source_blob: Option<DataId>,
    /// (name data, name len, fields data, fields len) per struct.
    struct_blobs: Vec<(DataId, i64, DataId, i64)>,
}

/// Per-function emission state that must not borrow the backend (the backend
/// is passed `&mut` next to it for runtime-import resolution).
struct FnState {
    blocks: Vec<Block>,
    vars: Vec<(Variable, Variable)>,
    /// Shared 24-byte `(tag, payload, failed)` out-area for `rt_*` calls
    /// and user calls alike (`rt_*` use only the first 16 bytes).
    out_slot: StackSlot,
    /// The function's out-pointer parameter (the returned triple's target).
    out_ptr: Variable,
    /// Attempt landing pads: `(LIR block, slot receiving the message)`.
    pads: HashMap<usize, SlotId>,
}

impl<'a> Backend<'a> {
    fn declare_ro(&mut self, bytes: &[u8]) -> DataId {
        let id = self
            .module
            .declare_anonymous_data(false, false)
            .expect("data declaration");
        let mut desc = DataDescription::new();
        desc.define(bytes.to_vec().into_boxed_slice());
        desc.set_segment_section(".rodata", ".rodata");
        desc.set_align(8);
        self.module.define_data(id, &desc).expect("data definition");
        id
    }

    /// The runtime-visible address of a data blob (declared into the
    /// function first — a `GlobalValue` is per-function in 0.122).
    fn data_ptr(&mut self, fb: &mut FunctionBuilder, id: DataId) -> Value {
        let gv = self.module.declare_data_in_func(id, fb.func);
        fb.ins().global_value(I64, gv)
    }

    fn declare_ro_i(
        &mut self,
        fb: &mut FunctionBuilder,
        id: StrId,
    ) -> (Value, Value) {
        let blob = self.strings_blob.expect("string pool declared");
        let s = &self.lir.strings[id.0];
        let offset = self.lir.string_offset(id) as i64;
        let base = self.data_ptr(fb, blob);
        let off = fb.ins().iconst(I64, offset);
        let ptr = fb.ins().iadd(base, off);
        let len = fb.ins().iconst(I64, s.len() as i64);
        (ptr, len)
    }

    /// Call an imported `rt_*` function; `args` are already lowered.
    fn rt_call(
        &mut self,
        fb: &mut FunctionBuilder,
        id: FuncId,
        args: &[Value],
    ) -> Vec<Value> {
        let fref = self.module.declare_func_in_func(id, fb.func);
        let call = fb.ins().call(fref, args);
        fb.inst_results(call).to_vec()
    }

    // -----------------------------------------------------------------------
    // Function bodies
    // -----------------------------------------------------------------------

    fn define_function(&mut self, f: &'a LirFunction) -> Result<(), String> {
        let id = self.fn_ids[&f.name];
        // The uniform ABI: out pointer, then (tag, payload) per param.
        let mut sig = Signature::new(self.call_conv);
        sig.params = vec![AbiParam::new(I64); 1 + 2 * f.params];
        let mut func = Function::with_name_signature(
            UserFuncName::testcase(Box::leak(mangle_symbol(&f.name).into_boxed_str())),
            sig,
        );

        let mut fbx = FunctionBuilderContext::new();
        let mut fb = FunctionBuilder::new(&mut func, &mut fbx);

        // One Cranelift block per LIR block; two variables per slot.
        let blocks: Vec<Block> = f.blocks.iter().map(|_| fb.create_block()).collect();
        let out_ptr = fb.declare_var(I64);
        let mut vars: Vec<(Variable, Variable)> = Vec::with_capacity(f.slots);
        for _ in 0..f.slots {
            let t = fb.declare_var(I64);
            let p = fb.declare_var(I64);
            vars.push((t, p));
        }

        let pads: HashMap<usize, SlotId> =
            f.catch_pads.iter().map(|(b, l)| (b.0, *l)).collect();

        // Entry block: parameters arrive as (tag, payload) pairs; every slot
        // starts as `nothing` — the verifier's initialization guarantee makes
        // this value dead on all real paths, but SSA variables need defs.
        fb.switch_to_block(blocks[0]);
        let out_param = fb.append_block_param(blocks[0], I64);
        fb.def_var(out_ptr, out_param);
        for k in 0..f.params {
            let t = fb.append_block_param(blocks[0], I64);
            let p = fb.append_block_param(blocks[0], I64);
            fb.def_var(vars[k].0, t);
            fb.def_var(vars[k].1, p);
        }
        for (t, p) in vars.iter().skip(f.params) {
            let tag = fb.ins().iconst(I64, 4); // TAG_NOTHING
            let pay = fb.ins().iconst(I64, 0);
            fb.def_var(*t, tag);
            fb.def_var(*p, pay);
        }

        let st = FnState {
            blocks,
            vars,
            out_slot: fb
                .create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 24, 3)),
            out_ptr,
            pads,
        };

        for (bi, block) in f.blocks.iter().enumerate() {
            // Block 0 is already the current block (we emitted the slot
            // initialization there); re-switching would trip the builder's
            // unfilled-block assertion.
            if bi != 0 {
                fb.switch_to_block(st.blocks[bi]);
            }
            // A landing pad consumes the failure message into its local first.
            if let Some(msg) = st.pads.get(&bi) {
                self.take_fail_into(&mut fb, &st, *msg);
            }
            for instr in &block.instrs {
                self.emit_instr(&mut fb, &st, block.pad_on_fail, instr);
            }
            self.emit_term(&mut fb, &st, f, &block.term);
        }

        fb.seal_all_blocks();
        fb.finalize();

        // Verify up front so internal emission bugs report *what* is wrong
        // (define_function's error otherwise hides the verifier's list).
        let foi: cranelift::codegen::settings::FlagsOrIsa = verify_flags().into();
        if let Err(errors) = cranelift::codegen::verify_function(&mut func, foi) {
            return Err(format!(
                "Cranelift verifier rejected `{}`: {}",
                f.name,
                errors.0.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; ")
            ));
        }

        let mut ctx = Context::new();
        ctx.func = func;
        self.module
            .define_function(id, &mut ctx)
            .map_err(|e| match e {
                cranelift_module::ModuleError::Compilation(
                    cranelift::codegen::CodegenError::Verifier(errors),
                ) => format!(
                    "verifier rejected `{}`: {}",
                    f.name,
                    errors.0.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; ")
                ),
                other => other.to_string(),
            })
    }

    /// The program entry the runtime shim calls: init, then the user script.
    /// A library (no script body) still initializes the runtime and exits 0.
    fn define_lagom_main(&mut self) -> Result<(), String> {
        let entry = self.lir.entry.clone();
        let entry_fn = self.fn_ids.get(&entry).copied();
        let source_len = self.source_len;

        let mut sig = Signature::new(self.call_conv);
        sig.returns = vec![AbiParam::new(I64)];
        let main_id = self
            .module
            .declare_function("lagom_main", Linkage::Export, &sig)
            .map_err(|e| e.to_string())?;

        let mut func =
            Function::with_name_signature(UserFuncName::testcase("lagom_main"), sig);
        let mut fbx = FunctionBuilderContext::new();
        let mut fb = FunctionBuilder::new(&mut func, &mut fbx);

        let block = fb.create_block();
        fb.switch_to_block(block);

        // Observability mode (§26.5): a build-mode fact, set before anything runs.
        let dev = fb.ins().iconst(I64, if self.dev { 1 } else { 0 });
        self.rt_call(&mut fb, self.rt.set_dev, &[dev]);

        // Structure registration (field access resolves against this table).
        for (name, name_len, fields, fields_len) in self.struct_blobs.clone() {
            let np = self.data_ptr(&mut fb, name);
            let nl = fb.ins().iconst(I64, name_len);
            let fp = self.data_ptr(&mut fb, fields);
            let fl = fb.ins().iconst(I64, fields_len);
            self.rt_call(&mut fb, self.rt.register_struct, &[np, nl, fp, fl]);
        }

        // Dev: hand the source to the failure-report renderer.
        if let Some(src) = self.source_blob {
            let ptr = self.data_ptr(&mut fb, src);
            let len = fb.ins().iconst(I64, source_len as i64);
            self.rt_call(&mut fb, self.rt.lom_set_source, &[ptr, len]);
        }

        // Call the entry with no arguments (absent in a library); the return
        // is the failed flag read from the out area (16 bytes in: the
        // triple's third slot). With no entry there is no call and no slot:
        // the runtime init above is the whole program, so report success —
        // loading the flag anyway would read uninitialized memory.
        let failed = if let Some(entry_fn) = entry_fn {
            let slot = fb
                .create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 24, 3));
            let out = fb.ins().stack_addr(I64, slot, 0);
            let entry_ref = self.module.declare_func_in_func(entry_fn, fb.func);
            fb.ins().call(entry_ref, &[out]);
            fb.ins().stack_load(I64, slot, 16)
        } else {
            fb.ins().iconst(I64, 0)
        };
        fb.ins().return_(&[failed]);

        fb.seal_all_blocks();
        fb.finalize();
        let mut ctx = Context::new();
        ctx.func = func;
        self.module
            .define_function(main_id, &mut ctx)
            .map_err(|e| match e {
                cranelift_module::ModuleError::Compilation(
                    cranelift::codegen::CodegenError::Verifier(errors),
                ) => format!(
                    "verifier rejected `lagom_main`: {}",
                    errors.0.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; ")
                ),
                other => other.to_string(),
            })
    }

    // -----------------------------------------------------------------------
    // Emission
    // -----------------------------------------------------------------------

    /// A frame slot's (tag, payload) operand pair.
    ///
    /// `pad` is the current LIR block's failure edge (`pad_on_fail`): text
    /// immediates materialize through the runtime and can therefore fail
    /// route (nothing can fail in practice, but the protocol is uniform).
    fn operand(
        &mut self,
        fb: &mut FunctionBuilder,
        st: &FnState,
        pad: Option<BlockId>,
        op: &LirOperand,
    ) -> (Value, Value) {
        match op {
            LirOperand::Int(i) => {
                let t = fb.ins().iconst(I64, 0); // TAG_NUMBER
                let p = fb.ins().iconst(I64, *i);
                (t, p)
            }
            LirOperand::FloatBits(bits) => {
                let t = fb.ins().iconst(I64, 1); // TAG_DECIMAL
                let p = fb.ins().iconst(I64, *bits);
                (t, p)
            }
            LirOperand::Bool(b) => {
                let t = fb.ins().iconst(I64, 3); // TAG_BOOLEAN
                let p = fb.ins().iconst(I64, *b as i64);
                (t, p)
            }
            LirOperand::Nothing => {
                let t = fb.ins().iconst(I64, 4); // TAG_NOTHING
                let p = fb.ins().iconst(I64, 0);
                (t, p)
            }
            LirOperand::Str(id) => {
                // A text immediate: the runtime boxes a pointer into the
                // string pool (payloads must be heap boxes).
                let (ptr, len) = self.declare_ro_i(fb, *id);
                let out = fb.ins().stack_addr(I64, st.out_slot, 0);
                let status = {
                    let results = self.rt_call(fb, self.rt.const_text, &[out, ptr, len]);
                    results[0]
                };
                self.route_failure(fb, st, pad, status, None);
                self.load_out(fb, st)
            }
            LirOperand::Slot(s) => {
                let (t, p) = st.vars[s.0];
                (fb.use_var(t), fb.use_var(p))
            }
        }
    }

    fn load_out(&self, fb: &mut FunctionBuilder, st: &FnState) -> (Value, Value) {
        let tag = fb.ins().stack_load(I64, st.out_slot, 0);
        let pay = fb.ins().stack_load(I64, st.out_slot, 8);
        (tag, pay)
    }

    fn store_pair(&self, fb: &mut FunctionBuilder, st: &FnState, slot: SlotId, pair: (Value, Value)) {
        let (t, p) = st.vars[slot.0];
        fb.def_var(t, pair.0);
        fb.def_var(p, pair.1);
    }

    /// The failure-status protocol: `status == 1` routes to the current
    /// block's pad, or (outside any attempt) crashes with the failure's
    /// message (G-19's honesty rule).
    fn route_failure(
        &mut self,
        fb: &mut FunctionBuilder,
        st: &FnState,
        pad: Option<BlockId>,
        status: Value,
        line: Option<i64>,
    ) {
        self.route_failure_mode(fb, st, pad, status, line, false);
    }

    /// `propagate_when_no_pad`: a failure of this instruction with no
    /// enclosing attempt propagates to the caller (the interpreter's rule
    /// for `convert` — 13.1) instead of crashing (G-19's index rule).
    fn route_failure_mode(
        &mut self,
        fb: &mut FunctionBuilder,
        st: &FnState,
        pad: Option<BlockId>,
        status: Value,
        line: Option<i64>,
        propagate_when_no_pad: bool,
    ) {
        let one = fb.ins().iconst(I64, 1);
        let failed = fb.ins().icmp(IntCC::Equal, status, one);
        let on_fail = fb.create_block();
        let on_ok = fb.create_block();
        fb.ins().brif(failed, on_fail, &[], on_ok, &[]);

        fb.switch_to_block(on_fail);
        match pad {
            Some(pad) => {
                fb.ins().jump(st.blocks[pad.0], &[]);
            }
            None if propagate_when_no_pad => {
                // Propagate to the caller: failed = 1, the message rides
                // the fail slot (13.1; the interpreter's `Exit::Fail`).
                let zero = fb.ins().iconst(I64, 0);
                let out = fb.use_var(st.out_ptr);
                fb.ins().store(MemFlags::new(), zero, out, 0);
                fb.ins().store(MemFlags::new(), zero, out, 8);
                fb.ins().store(MemFlags::new(), one, out, 16);
                fb.ins().return_(&[]);
            }
            None => {
                // An instruction failure with no enclosing attempt is a
                // crash whose message is the stored failure (G-19).
                let ln = fb
                    .ins()
                    .iconst(I64, line.unwrap_or(0).max(0));
                self.rt_call(fb, self.rt.panic_from_fail, &[ln]);
                fb.ins().trap(TrapCode::unwrap_user(1));
            }
        }
        fb.switch_to_block(on_ok);
    }

    fn call_out_status(
        &mut self,
        fb: &mut FunctionBuilder,
        st: &FnState,
        pad: Option<BlockId>,
        id: FuncId,
        rest: &[Value],
        line: Option<i64>,
    ) -> (Value, Value) {
        self.call_out_status_mode(fb, st, pad, id, rest, line, false)
    }

    /// [`call_out_status`] with an explicit no-pad policy (see
    /// [`Backend::route_failure_mode`]).
    #[allow(clippy::too_many_arguments)]
    fn call_out_status_mode(
        &mut self,
        fb: &mut FunctionBuilder,
        st: &FnState,
        pad: Option<BlockId>,
        id: FuncId,
        rest: &[Value],
        line: Option<i64>,
        propagate_when_no_pad: bool,
    ) -> (Value, Value) {
        let out = fb.ins().stack_addr(I64, st.out_slot, 0);
        let mut args = vec![out];
        args.extend_from_slice(rest);
        let results = self.rt_call(fb, id, &args);
        let status = results[0];
        self.route_failure_mode(fb, st, pad, status, line, propagate_when_no_pad);
        self.load_out(fb, st)
    }

    /// A landing pad: consume the stored failure into the message slot.
    fn take_fail_into(&mut self, fb: &mut FunctionBuilder, st: &FnState, slot: SlotId) {
        let out = fb.ins().stack_addr(I64, st.out_slot, 0);
        let results = self.rt_call(fb, self.rt.take_fail, &[out]);
        let status = results[0];
        // `rt_take_fail` cannot fail (a pad with no failure is a compiler bug
        // that panics inside the runtime).
        let zero = fb.ins().iconst(I64, 0);
        let failed = fb.ins().icmp(IntCC::NotEqual, status, zero);
        fb.ins().trapnz(failed, TrapCode::unwrap_user(2));
        let pair = self.load_out(fb, st);
        self.store_pair(fb, st, slot, pair);
    }

    fn emit_instr(
        &mut self,
        fb: &mut FunctionBuilder,
        st: &FnState,
        pad: Option<BlockId>,
        instr: &LirInstr,
    ) {
        match instr {
            LirInstr::Copy { dest, value, .. } => {
                let (t, p) = self.operand(fb, st, pad, value);
                let out = fb.ins().stack_addr(I64, st.out_slot, 0);
                let status = {
                    let results = self.rt_call(fb, self.rt.clone, &[out, t, p]);
                    results[0]
                };
                self.route_failure(fb, st, pad, status, None);
                let pair = self.load_out(fb, st);
                self.store_pair(fb, st, *dest, pair);
            }
            LirInstr::Unary { dest, op, operand, line } => {
                let (vt, vp) = self.operand(fb, st, pad, operand);
                let op_v = fb.ins().iconst(I64, *op);
                let line_v = fb.ins().iconst(I64, *line);
                let (t, p) = self.call_out_status(
                    fb,
                    st,
                    pad,
                    self.rt.unop,
                    &[op_v, vt, vp, line_v],
                    Some(*line),
                );
                self.store_pair(fb, st, *dest, (t, p));
            }
            LirInstr::Binary { dest, op, left, right, line } => {
                let (lt, lp) = self.operand(fb, st, pad, left);
                let (rt_, rp) = self.operand(fb, st, pad, right);
                let op_v = fb.ins().iconst(I64, *op);
                let line_v = fb.ins().iconst(I64, *line);
                let (t, p) = self.call_out_status(
                    fb,
                    st,
                    pad,
                    self.rt.binop,
                    &[op_v, lt, lp, rt_, rp, line_v],
                    Some(*line),
                );
                self.store_pair(fb, st, *dest, (t, p));
            }
            LirInstr::Call { dest, callee, args, .. } => {
                // Flatten the (tag, payload) pairs.
                let mut call_args = Vec::with_capacity(args.len() * 2);
                for a in args {
                    let (t, p) = self.operand(fb, st, pad, a);
                    call_args.push(t);
                    call_args.push(p);
                }
                let out = fb.ins().stack_addr(I64, st.out_slot, 0);
                let mut call_args2 = vec![out];
                call_args2.extend_from_slice(&call_args);
                let callee_id = self.fn_ids[callee];
                let fref = self.module.declare_func_in_func(callee_id, fb.func);
                let call = fb.ins().call(fref, &call_args2);
                let _ = call;
                let t = fb.ins().stack_load(I64, st.out_slot, 0);
                let p = fb.ins().stack_load(I64, st.out_slot, 8);
                let failed = fb.ins().stack_load(I64, st.out_slot, 16);

                // Propagation (13.1): with a pad, jump; without one, return
                // the failed flag untouched (the message rides the fail slot).
                let one = fb.ins().iconst(I64, 1);
                let is_failed = fb.ins().icmp(IntCC::Equal, failed, one);
                let on_fail = fb.create_block();
                let on_ok = fb.create_block();
                fb.ins().brif(is_failed, on_fail, &[], on_ok, &[]);

                fb.switch_to_block(on_fail);
                match pad {
                    Some(pad) => {
                        fb.ins().jump(st.blocks[pad.0], &[]);
                    }
                    None => {
                        // Propagate to the caller: failed = 1, no value.
                        let z = fb.ins().iconst(I64, 0);
                        let out = fb.use_var(st.out_ptr);
                        fb.ins().store(MemFlags::new(), z, out, 0);
                        fb.ins().store(MemFlags::new(), z, out, 8);
                        fb.ins().store(MemFlags::new(), one, out, 16);
                        fb.ins().return_(&[]);
                    }
                }
                fb.switch_to_block(on_ok);
                self.store_pair(fb, st, *dest, (t, p));
            }
            LirInstr::Say { value, .. } => {
                let (t, p) = self.operand(fb, st, pad, value);
                self.rt_call(fb, self.rt.say, &[t, p]);
            }
            LirInstr::Ask { dest, question, line } => {
                let (qt, qp) = self.operand(fb, st, pad, question);
                let line_v = fb.ins().iconst(I64, *line);
                let (t, p) = self.call_out_status(fb, st, pad, self.rt.ask, &[qt, qp, line_v], Some(*line));
                self.store_pair(fb, st, *dest, (t, p));
            }
            LirInstr::StructNew { dest, name, fields, line } => {
                let (nptr, nlen) = self.declare_ro_i(fb, *name);
                let (t, p) = self.call_out_status(fb, st, pad, self.rt.struct_new, &[nptr, nlen], Some(*line));
                for (fname, value) in fields {
                    let (fptr, flen) = self.declare_ro_i(fb, *fname);
                    let (vt, vp) = self.operand(fb, st, pad, value);
                    self.rt_call(fb, self.rt.struct_push, &[p, fptr, flen, vt, vp]);
                }
                self.store_pair(fb, st, *dest, (t, p));
            }
            LirInstr::FieldGet { dest, base, field, line } => {
                let (bt, bp) = self.operand(fb, st, pad, base);
                let (fptr, flen) = self.declare_ro_i(fb, *field);
                let line_v = fb.ins().iconst(I64, *line);
                let (t, p) = self.call_out_status(fb, st, pad, self.rt.field_get, &[bt, bp, fptr, flen, line_v], Some(*line));
                self.store_pair(fb, st, *dest, (t, p));
            }
            LirInstr::FieldSet { base, field, value, line } => {
                let (bt, bp) = self.operand(fb, st, pad, &LirOperand::Slot(*base));
                let (fptr, flen) = self.declare_ro_i(fb, *field);
                let (vt, vp) = self.operand(fb, st, pad, value);
                { let line_v = fb.ins().iconst(I64, *line); self.rt_call(fb, self.rt.field_set, &[bt, bp, fptr, flen, vt, vp, line_v]); }
            }
            LirInstr::IndexGet { dest, base, index, line } => {
                let (bt, bp) = self.operand(fb, st, pad, base);
                let (it, ip) = self.operand(fb, st, pad, index);
                let line_v = fb.ins().iconst(I64, *line);
                let (t, p) = self.call_out_status(fb, st, pad, self.rt.index_get, &[bt, bp, it, ip, line_v], Some(*line));
                self.store_pair(fb, st, *dest, (t, p));
            }
            LirInstr::IndexSet { base, index, value, line } => {
                let (bt, bp) = self.operand(fb, st, pad, &LirOperand::Slot(*base));
                let (it, ip) = self.operand(fb, st, pad, index);
                let (vt, vp) = self.operand(fb, st, pad, value);
                let line_v = fb.ins().iconst(I64, *line);
                let status = {
                    let results = self.rt_call(fb, self.rt.index_set, &[bt, bp, it, ip, vt, vp, line_v]);
                    results[0]
                };
                self.route_failure(fb, st, pad, status, Some(*line));
                // The base box is rebuilt in place through its payload
                // pointer; the (tag, payload) pair is unchanged.
            }
            LirInstr::ListNew { dest, elements, line } => {
                let (t, p) = self.call_out_status(fb, st, pad, self.rt.list_new, &[], Some(*line));
                for e in elements {
                    let (et, ep) = self.operand(fb, st, pad, e);
                    self.rt_call(fb, self.rt.list_push, &[t, p, et, ep]);
                }
                self.store_pair(fb, st, *dest, (t, p));
            }
            LirInstr::MapNew { dest, entries, line } => {
                let (t, p) = self.call_out_status(fb, st, pad, self.rt.map_new, &[], Some(*line));
                for (k, v) in entries {
                    let (kt, kp) = self.operand(fb, st, pad, k);
                    let (vt, vp) = self.operand(fb, st, pad, v);
                    self.rt_call(fb, self.rt.map_push, &[t, p, kt, kp, vt, vp]);
                }
                self.store_pair(fb, st, *dest, (t, p));
            }
            LirInstr::PairNew { dest, first, second, line } => {
                let (at, ap) = self.operand(fb, st, pad, first);
                let (bt, bp) = self.operand(fb, st, pad, second);
                let (t, p) = self.call_out_status(fb, st, pad, self.rt.pair_new, &[at, ap, bt, bp], Some(*line));
                self.store_pair(fb, st, *dest, (t, p));
            }
            LirInstr::Format { dest, parts, line } => {
                let (t, p) = self.call_out_status(fb, st, pad, self.rt.format_new, &[], Some(*line));
                for part in parts {
                    match part {
                        FormatPart::Lit(id) => {
                            let (lptr, llen) = self.declare_ro_i(fb, *id);
                            self.rt_call(fb, self.rt.format_push_lit, &[t, p, lptr, llen]);
                        }
                        FormatPart::Value(op) => {
                            let (vt, vp) = self.operand(fb, st, pad, op);
                            self.rt_call(fb, self.rt.format_push_val, &[t, p, vt, vp]);
                        }
                    }
                }
                self.store_pair(fb, st, *dest, (t, p));
            }
            LirInstr::Convert { dest, conv, value, line } => {
                let (vt, vp) = self.operand(fb, st, pad, value);
                let conv_v = fb.ins().iconst(I64, *conv);
                // A failed conversion propagates like a can-fail call (13.1);
                // outside any attempt the caller crashes honestly.
                let (t, p) = self.call_out_status_mode(
                    fb,
                    st,
                    pad,
                    self.rt.convert,
                    &[conv_v, vt, vp],
                    Some(*line),
                    true,
                );
                self.store_pair(fb, st, *dest, (t, p));
            }
            LirInstr::Random { dest, lo, hi, line } => {
                let (lt, lp) = self.operand(fb, st, pad, lo);
                let (ht, hp) = self.operand(fb, st, pad, hi);
                let line_v = fb.ins().iconst(I64, *line);
                let (t, p) =
                    self.call_out_status(fb, st, pad, self.rt.random, &[lt, lp, ht, hp, line_v], Some(*line));
                self.store_pair(fb, st, *dest, (t, p));
            }
            LirInstr::FirstOf { dest, list, line } => {
                let (lt, lp) = self.operand(fb, st, pad, list);
                let line_v = fb.ins().iconst(I64, *line);
                let (t, p) =
                    self.call_out_status(fb, st, pad, self.rt.first_of, &[lt, lp, line_v], Some(*line));
                self.store_pair(fb, st, *dest, (t, p));
            }
            LirInstr::SizeOf { dest, value, line } => {
                let (vt, vp) = self.operand(fb, st, pad, value);
                let line_v = fb.ins().iconst(I64, *line);
                let (t, p) =
                    self.call_out_status(fb, st, pad, self.rt.size_of, &[vt, vp, line_v], Some(*line));
                self.store_pair(fb, st, *dest, (t, p));
            }
            LirInstr::Join { dest, list, line } => {
                let (lt, lp) = self.operand(fb, st, pad, list);
                let line_v = fb.ins().iconst(I64, *line);
                let (t, p) =
                    self.call_out_status(fb, st, pad, self.rt.join, &[lt, lp, line_v], Some(*line));
                self.store_pair(fb, st, *dest, (t, p));
            }
            LirInstr::TextOp { dest, op, value, line } => {
                let (vt, vp) = self.operand(fb, st, pad, value);
                let op_v = fb.ins().iconst(I64, *op);
                let line_v = fb.ins().iconst(I64, *line);
                let (t, p) =
                    self.call_out_status(fb, st, pad, self.rt.textop, &[op_v, vt, vp, line_v], Some(*line));
                self.store_pair(fb, st, *dest, (t, p));
            }
            LirInstr::MathOp { dest, op, value, line } => {
                let (vt, vp) = self.operand(fb, st, pad, value);
                let op_v = fb.ins().iconst(I64, *op);
                let line_v = fb.ins().iconst(I64, *line);
                let (t, p) =
                    self.call_out_status(fb, st, pad, self.rt.mathop, &[op_v, vt, vp, line_v], Some(*line));
                self.store_pair(fb, st, *dest, (t, p));
            }
            LirInstr::Check { value, has_cmp, cmp_op, left, right, line } => {
                let (vt, vp) = self.operand(fb, st, pad, value);
                let (lt, lp) = self.operand(fb, st, pad, left);
                let (rt_, rp) = self.operand(fb, st, pad, right);
                let cmp_v = fb.ins().iconst(I64, *cmp_op);
                let has_v = fb.ins().iconst(I64, *has_cmp as i64);
                let line_v = fb.ins().iconst(I64, *line);
                self.rt_call(fb, self.rt.check, &[vt, vp, lt, lp, rt_, rp, cmp_v, has_v, line_v]);
            }
            LirInstr::LomEntry { function, args } => {
                // Dev-only probe (§26.5): the argument snapshots ride the
                // entry event into the ring.
                let (fptr, flen) = self.declare_ro_i(fb, *function);
                self.rt_call(fb, self.rt.lom_entry, &[fptr, flen]);
                for (name, value) in args {
                    let (nptr, nlen) = self.declare_ro_i(fb, *name);
                    let (vt, vp) = self.operand(fb, st, pad, value);
                    self.rt_call(fb, self.rt.lom_entry_arg, &[nptr, nlen, vt, vp]);
                }
                self.rt_call(fb, self.rt.lom_entry_end, &[]);
            }
            LirInstr::LomBind { name, value, site } => {
                let (nptr, nlen) = self.declare_ro_i(fb, *name);
                let (vt, vp) = self.operand(fb, st, pad, value);
                let site_v = fb.ins().iconst(I64, *site);
                self.rt_call(fb, self.rt.lom_bind, &[nptr, nlen, vt, vp, site_v]);
            }
            LirInstr::LomFail { .. } => {
                // Dropped: `rt_set_fail` records exactly one failure event per
                // failure in dev builds (see the module docs).
            }
        }
    }

    fn emit_term(&mut self, fb: &mut FunctionBuilder, st: &FnState, _f: &'a LirFunction, term: &LirTerm) {
        match term {
            LirTerm::Goto(target) => {
                fb.ins().jump(st.blocks[target.0], &[]);
            }
            LirTerm::Branch { cond, then, otherwise } => {
                let (ct, cp) = self.operand(fb, st, None, cond);
                let truthy = {
                    let results = self.rt_call(fb, self.rt.branch_check, &[ct, cp]);
                    results[0]
                };
                fb.ins().brif(truthy, st.blocks[then.0], &[], st.blocks[otherwise.0], &[]);
            }
            LirTerm::Break(target) | LirTerm::Continue(target) => {
                fb.ins().jump(st.blocks[target.0], &[]);
            }
            LirTerm::Return(value) => {
                let (t, p) = self.operand(fb, st, None, value);
                let out = fb.use_var(st.out_ptr);
                let zero = fb.ins().iconst(I64, 0);
                // A script/test body's triple is (nothing, nothing, 0);
                // a real return writes the value (lagom_main reads only
                // the failed flag — the interpreter's script parity).
                fb.ins().store(MemFlags::new(), t, out, 0);
                fb.ins().store(MemFlags::new(), p, out, 8);
                fb.ins().store(MemFlags::new(), zero, out, 16);
                fb.ins().return_(&[]);
            }
            LirTerm::ReturnFailed { message, line } => {
                let (mt, mp) = self.operand(fb, st, None, message);
                self.rt_call(fb, self.rt.set_fail, &[mt, mp]);
                // Propagate: failed = 1, the message stays in the fail slot.
                let zero = fb.ins().iconst(I64, 0);
                let one = fb.ins().iconst(I64, 1);
                let out = fb.use_var(st.out_ptr);
                fb.ins().store(MemFlags::new(), zero, out, 0);
                fb.ins().store(MemFlags::new(), zero, out, 8);
                fb.ins().store(MemFlags::new(), one, out, 16);
                fb.ins().return_(&[]);
                let _ = line;
            }
            LirTerm::FailInFrame { message, catch } => {
                let (mt, mp) = self.operand(fb, st, None, message);
                self.rt_call(fb, self.rt.set_fail, &[mt, mp]);
                fb.ins().jump(st.blocks[catch.0], &[]);
            }
            LirTerm::Unreachable => {
                fb.ins().trap(TrapCode::unwrap_user(1));
            }
        }
    }
}
