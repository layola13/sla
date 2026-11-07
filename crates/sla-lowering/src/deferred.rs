//! Registry of lowering_rules.zig rule families that lane D has NOT
//! ported (vertical-slice scope). Callers that reach one of these
//! families must fail with [`LoweringError::Unsupported`] naming the
//! family — never approximate silently. Line ranges are into
//! `~/workspace/local-sa/sa_plugin_sla/src/lowering_rules.zig` unless
//! noted; they are survey-grade (family starts), kept so the deferred
//! work can be picked up without re-surveying the file.

use crate::LoweringError;

/// One unported rule family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeferredFamily {
    pub name: &'static str,
    pub zig_ref: &'static str,
    pub note: &'static str,
}

/// The deferred inventory. The line summary delivered with this lane
/// repeats this list as the "uncovered rules"清单.
pub const DEFERRED_FAMILIES: &[DeferredFamily] = &[
    DeferredFamily {
        name: "async continuation plans",
        zig_ref: "lowering_rules.zig (planAsync*Continuation family)",
        note: "Join2/two/linear/general/loop/single await continuation plans and the async function-return plan; drives separate genAsync* emitters in codegen.",
    },
    DeferredFamily {
        name: "future/task/executor/poll runtime-call plans",
        zig_ref: "lowering_rules.zig (planFutureRuntimeCall, planTaskRuntimeCall, planExecutorRuntimeCall)",
        note: "Runtime EXPAND shapes for Future/Task/Executor/Poll surfaces.",
    },
    DeferredFamily {
        name: "imported-macro call plans",
        zig_ref: "lowering_rules.zig (planImportedMacroCall, ImportedMacroArg plans)",
        note: "Needs the checker's imported-macro tables; codegen fails loudly when a macro call is not println.",
    },
    DeferredFamily {
        name: "resolved static call plans",
        zig_ref: "lowering_rules.zig:5819-5910 (planStaticCall, planResolvedStaticCall, planResolvedStaticCallLowering)",
        note: "Monomorphized/operator-resolved calls keyed by tc.resolved_call_symbols; CheckInfo exposes the symbol, the lowering itself is deferred.",
    },
    DeferredFamily {
        name: "dyn coercion / dyn dispatch plans",
        zig_ref: "lowering_rules.zig (planDynCoercion, dyn dispatch + vtable name helpers)",
        note: "Trait-object coercions and dynamic dispatch slot resolution.",
    },
    DeferredFamily {
        name: "refcell handle plans (beyond call-arg lifecycle)",
        zig_ref: "lowering_rules.zig (planRefCellHandleRelease, planRefCellValueStateTransfer, planRefCellLoopStateMerge, ...)",
        note: "The RefCell borrow-handle state machine; the small call-arg lifecycle and borrow-address-temp plans ARE ported (call_args.rs). planRefCellLoopStateMerge's restoresPreLoop fact is consumed by the loop emitters as a no-op restore in the slice because no RefCell state is tracked.",
    },
    DeferredFamily {
        name: "while-let / let-pattern plans",
        zig_ref: "lowering_rules.zig:690-795 (WhileLetPatternPlan family)",
        note: "Pattern-kind classification exists as types there; the while-let and if-let emitters are deferred in codegen-text.",
    },
    DeferredFamily {
        name: "scalar match guard plans",
        zig_ref: "lowering_rules.zig:926-984 (supportsScalarMatchGuard, scalarMatchGuardTempCount)",
        note: "Match/switch guard temp accounting; no match/switch in the slice.",
    },
    DeferredFamily {
        name: "struct literal / field layout plans",
        zig_ref: "lowering_rules.zig (planStructLiteralField, planStructLiteralFieldTransfer, fieldLayout family)",
        note: "Aggregate layout is checker-side data; struct literal emission is deferred in codegen-text.",
    },
    DeferredFamily {
        name: "spaceship plans",
        zig_ref: "lowering_rules.zig:5667+ (SpaceshipPlan, planSpaceship)",
        note: "`<=>` scalar planning returns None in planScalarBinaryOp (ported); the struct/numeric Ordering emitter is deferred.",
    },
    DeferredFamily {
        name: "macro param consumption",
        zig_ref: "control_flow_rules.zig (macroParamConsumption family)",
        note: "Macro-body param consumption analysis; user-macro inlining is deferred in codegen-text.",
    },
    DeferredFamily {
        name: "collection element-type extraction",
        zig_ref: "lowering_rules.zig (vecElementType, hashMapTypes, set element extractors)",
        note: "Type-shape extraction over user-defined names; used by collection emitters, none in the slice.",
    },
];

/// Build the loud error for a deferred family by name.
pub fn deferred_error(name: &str) -> LoweringError {
    for family in DEFERRED_FAMILIES {
        if family.name == name {
            return LoweringError::Unsupported {
                feature: family.name,
                zig_ref: family.zig_ref,
            };
        }
    }
    LoweringError::Unsupported {
        feature: "unknown deferred family",
        zig_ref: "lowering_rules.zig",
    }
}
