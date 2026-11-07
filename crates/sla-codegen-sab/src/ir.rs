//! SAB intermediate representation: the instruction, signature and const
//! types the encoder serializes.
//!
//! These types mirror, field for field, the sci definitions the Zig
//! codegen produces:
//! - `Instruction` / `Operand` / `InstKind` / `OpKind` / `OpCode` /
//!   `CapPrefix` / `PrimType`: sci `src/common/instruction.zig`
//! - `FunctionSig` / `ParamSpec` / `FunctionKind`: sci
//!   `src/common/signature.zig`
//! - `ConstDecl` / `ConstValue`: sci `src/common/const_decl.zig`
//! - `AtomicOrdering` / `AtomicRmwOp`: sci `src/common/atomic.zig`
//!
//! Every enum's ordinal (its `#[repr(u8)]` value) is the declaration
//! order of the Zig enum, because the SAB wire format stores
//! `@intFromEnum` values. Do not reorder variants.
//!
//! Deliberate omission: Zig's `Instruction.raw_text` and
//! `ConstDecl.raw_text` are *not* modeled. The SAB write path never
//! serializes them (the decoder re-synthesizes an equivalent text), so
//! they cannot affect emitted bytes. The text form lives in
//! `sla-codegen-text`; SAB is the production path.

/// Source position attached to instructions, sigs and const decls
/// (sci `src/common/upstream.zig` `UpstreamLoc`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpstreamLoc {
    pub file: String,
    pub line: u32,
    pub col: u32,
}

/// Capability prefix of a parameter or return value (instruction.zig).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CapPrefix {
    ByValue = 0,
    Borrow = 1,
    Move = 2,
    Raw = 3,
}

/// Primitive types (instruction.zig `PrimType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PrimType {
    Void = 0,
    I1 = 1,
    I8 = 2,
    I16 = 3,
    I32 = 4,
    I64 = 5,
    U8 = 6,
    U16 = 7,
    U32 = 8,
    U64 = 9,
    F32 = 10,
    F64 = 11,
    Ptr = 12,
    BlobHandle = 13,
    V128 = 14,
}

/// Instruction kinds (instruction.zig `InstKind`, 36 variants).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InstKind {
    Alloc = 0,
    StackAlloc = 1,
    Load = 2,
    Store = 3,
    AtomicLoad = 4,
    AtomicStore = 5,
    Cmpxchg = 6,
    AtomicRmw = 7,
    Fence = 8,
    Borrow = 9,
    Move = 10,
    Release = 11,
    Assign = 12,
    Op = 13,
    PtrAdd = 14,
    Jmp = 15,
    Br = 16,
    BrNull = 17,
    Call = 18,
    CallIndirect = 19,
    Try = 20,
    EarlyReturn = 21,
    Panic = 22,
    PanicMsg = 23,
    Return = 24,
    Take = 25,
    FuncDecl = 26,
    FfiWrapperDecl = 27,
    ExternDecl = 28,
    ExportDecl = 29,
    TestDecl = 30,
    Label = 31,
    RawCast = 32,
    AssumeSafe = 33,
    AssumeBorrow = 34,
    Native = 35,
}

/// Arithmetic / conversion operation kinds (instruction.zig `OpKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OpKind {
    Add = 0,
    Sub = 1,
    Mul = 2,
    Sdiv = 3,
    Udiv = 4,
    Srem = 5,
    Urem = 6,
    Neg = 7,
    And = 8,
    Or = 9,
    Xor = 10,
    Shl = 11,
    Lshr = 12,
    Ashr = 13,
    Not = 14,
    Eq = 15,
    Ne = 16,
    Slt = 17,
    Sle = 18,
    Sgt = 19,
    Sge = 20,
    Ult = 21,
    Ule = 22,
    Ugt = 23,
    Uge = 24,
    Div = 25,
    Rem = 26,
    Gt = 27,
    Lt = 28,
    Shr = 29,
    Fadd = 30,
    Fsub = 31,
    Fmul = 32,
    Fdiv = 33,
    Fneg = 34,
    FcmpEq = 35,
    FcmpNe = 36,
    FcmpLt = 37,
    FcmpLe = 38,
    FcmpGt = 39,
    FcmpGe = 40,
    Trunc = 41,
    Zext = 42,
    Sext = 43,
    Fptosi = 44,
    Sitofp = 45,
    Uitofp = 46,
    Fptrunc = 47,
    Fpext = 48,
    Bitcast = 49,
    AddV128 = 50,
    SubV128 = 51,
    MulV128 = 52,
    ShuffleV128 = 53,
    ExtractLane = 54,
    InsertLane = 55,
}

/// SA text-level op codes carried by `Operand::OpCode`
/// (instruction.zig `OpCode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OpCode {
    Add = 0,
    Sub = 1,
    Mul = 2,
    Div = 3,
    Sdiv = 4,
    Udiv = 5,
    Rem = 6,
    Srem = 7,
    Urem = 8,
    Gt = 9,
    Lt = 10,
    Sgt = 11,
    Slt = 12,
    Sge = 13,
    Sle = 14,
    Ugt = 15,
    Ult = 16,
    Uge = 17,
    Ule = 18,
    Eq = 19,
    Ne = 20,
    And = 21,
    Or = 22,
    Shl = 23,
    Shr = 24,
}

/// Atomic memory orderings (sci `common/atomic.zig`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AtomicOrdering {
    Relaxed = 0,
    Acquire = 1,
    Release = 2,
    AcqRel = 3,
    SeqCst = 4,
}

/// Atomic read-modify-write operations (sci `common/atomic.zig`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AtomicRmwOp {
    Add = 0,
    Sub = 1,
    And = 2,
    Or = 3,
    Xor = 4,
    Xchg = 5,
    Min = 6,
    Max = 7,
    Umin = 8,
    Umax = 9,
}

/// One instruction operand (instruction.zig `Operand`).
///
/// `Reg` / `Symbol` / `Label` / `Func` carry ids into the module-wide
/// symbol table — in the Zig codegen a single interning namespace serves
/// register names, function names and labels alike, and the SAB operands
/// store those ids directly. `Text` / `NativeText` carry the string
/// itself; the encoder pools it at serialization time.
#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    None,
    Reg(u32),
    Symbol(u32),
    Label(u32),
    Func(u32),
    ImmI64(i64),
    ImmU64(u64),
    ImmInt(i128),
    ImmFloat(f64),
    OpCode(OpCode),
    CapPrefix(CapPrefix),
    Offset(u32),
    Ty(u32),
    Text(String),
    NativeText(String),
}

impl Operand {
    /// The operand kind tag byte written before the payload
    /// (sab.zig `operandTag`).
    pub fn tag(&self) -> u8 {
        match self {
            Operand::None => 0,
            Operand::Reg(_) => 1,
            Operand::Symbol(_) => 2,
            Operand::Label(_) => 3,
            Operand::Func(_) => 4,
            Operand::ImmI64(_) => 5,
            Operand::ImmU64(_) => 6,
            Operand::ImmInt(_) => 7,
            Operand::ImmFloat(_) => 8,
            Operand::OpCode(_) => 9,
            Operand::CapPrefix(_) => 10,
            Operand::Offset(_) => 11,
            Operand::Ty(_) => 12,
            Operand::Text(_) => 13,
            Operand::NativeText(_) => 14,
        }
    }
}

/// One SAB instruction (instruction.zig `Instruction`, minus the
/// non-serialized `raw_text`; see the module docs).
#[derive(Debug, Clone, PartialEq)]
pub struct Instruction {
    pub kind: InstKind,
    pub source_line: u32,
    pub expanded_line: u32,
    pub package_identity: Option<String>,
    pub package_source_sha256: Option<[u8; 32]>,
    pub upstream_loc: Option<UpstreamLoc>,
    pub op_kind: Option<OpKind>,
    /// Always exactly four slots; unused slots are `Operand::None`.
    pub operands: [Operand; 4],
    pub atomic_value_ty: Option<u32>,
    pub atomic_ordering: Option<AtomicOrdering>,
    pub atomic_second_ordering: Option<AtomicOrdering>,
    pub atomic_rmw_op: Option<AtomicRmwOp>,
    pub atomic_expected_text: Option<String>,
    pub atomic_new_text: Option<String>,
    pub native_reg_names: Vec<String>,
}

impl Instruction {
    /// Mirrors `inst.makeInstruction(kind, source_line, expanded_line, ..)`
    /// as used by `Codegen.makeInst`: all optional fields empty, all
    /// operands `None`.
    pub fn new(kind: InstKind, source_line: u32, expanded_line: u32) -> Self {
        Instruction {
            kind,
            source_line,
            expanded_line,
            package_identity: None,
            package_source_sha256: None,
            upstream_loc: None,
            op_kind: None,
            operands: [Operand::None, Operand::None, Operand::None, Operand::None],
            atomic_value_ty: None,
            atomic_ordering: None,
            atomic_second_ordering: None,
            atomic_rmw_op: None,
            atomic_expected_text: None,
            atomic_new_text: None,
            native_reg_names: Vec::new(),
        }
    }
}

/// Function kinds (signature.zig `FunctionKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FunctionKind {
    Normal = 0,
    FfiWrapper = 1,
    External = 2,
    Exported = 3,
    TestFunc = 4,
}

/// One function parameter (signature.zig `ParamSpec`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamSpec {
    pub name: String,
    pub ty: PrimType,
    pub cap: CapPrefix,
}

/// A function signature (signature.zig `FunctionSig`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSig {
    pub id: u32,
    pub name: String,
    pub params: Vec<ParamSpec>,
    pub kind: FunctionKind,
    pub return_cap: Option<CapPrefix>,
    pub return_ty: PrimType,
    pub return_fallible: bool,
    pub entry_inst_idx: u32,
    pub is_ffi_wrapper: bool,
    pub upstream_file: Option<String>,
    pub upstream_loc: Option<UpstreamLoc>,
    pub param_ids: Vec<u32>,
    pub reg_ids: Vec<u32>,
    pub llvm_name: Option<String>,
    pub ignored: bool,
    pub should_panic: bool,
}

impl FunctionSig {
    /// A plain `normal` signature with the defaults the Zig struct uses.
    pub fn new(id: u32, name: &str, return_ty: PrimType, entry_inst_idx: u32) -> Self {
        FunctionSig {
            id,
            name: name.to_string(),
            params: Vec::new(),
            kind: FunctionKind::Normal,
            return_cap: None,
            return_ty,
            return_fallible: false,
            entry_inst_idx,
            is_ffi_wrapper: false,
            upstream_file: None,
            upstream_loc: None,
            param_ids: Vec::new(),
            reg_ids: Vec::new(),
            llvm_name: None,
            ignored: false,
            should_panic: false,
        }
    }
}

/// Bytes payload of a bytes-like const value (const_decl.zig
/// `BytesLiteral`): raw data, optionally with a trailing repeat segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytesLiteral {
    pub bytes: Vec<u8>,
    /// `(count, byte)` trailing repeat, mirroring Zig's
    /// `repeat_count: ?u64` / `repeat_byte: ?u8` pair (absent = no
    /// repeat segment; the Zig writer emits byte 0 when only the
    /// count is present).
    pub repeat: Option<(u64, u8)>,
}

/// One field of a struct const value (const_decl.zig `StructField`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructField {
    pub name: String,
    pub size: u32,
    pub value: ConstValue,
}

/// One slot of a vtable const value (const_decl.zig `VTableSlot`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VTableSlot {
    pub name: String,
    pub func_name: String,
}

/// A const value (const_decl.zig `ConstValue`, tagged by
/// `ConstLiteralKind`: hex=0, utf8=1, repeat=2, struct_=3, vtable=4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstValue {
    Hex(BytesLiteral),
    Utf8(BytesLiteral),
    Repeat(BytesLiteral),
    Struct(Vec<StructField>),
    VTable(Vec<VTableSlot>),
}

impl ConstValue {
    /// The `ConstLiteralKind` tag byte written before the payload.
    pub fn tag(&self) -> u8 {
        match self {
            ConstValue::Hex(_) => 0,
            ConstValue::Utf8(_) => 1,
            ConstValue::Repeat(_) => 2,
            ConstValue::Struct(_) => 3,
            ConstValue::VTable(_) => 4,
        }
    }
}

/// A const declaration (const_decl.zig `ConstDecl`, minus `raw_text`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstDecl {
    pub source_line: u32,
    pub expanded_line: u32,
    pub upstream_loc: Option<UpstreamLoc>,
    pub name: String,
    pub literal_text: String,
    pub value: ConstValue,
}

/// A complete module ready for the encoder: the four SAB sections'
/// logical content, in the exact shape `sab.encodeProgramWithConsts`
/// receives them from the Zig codegen.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Module {
    /// The codegen's intern table, in first-intern order. Operand
    /// `Reg`/`Symbol`/`Label`/`Func` ids index into this list.
    pub symbols: Vec<String>,
    pub function_sigs: Vec<FunctionSig>,
    pub const_decls: Vec<ConstDecl>,
    pub instructions: Vec<Instruction>,
}
