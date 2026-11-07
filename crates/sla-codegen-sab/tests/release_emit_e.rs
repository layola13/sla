//! E7 release-emission integration tests (moved out of
//! release_emit.rs by the 700-line rule; same assertions).

use sla_ast::{Param, Primitive, Type};
use sla_codegen_sab::error::Error;
use sla_codegen_sab::ir::{CapPrefix, InstKind};
use sla_codegen_sab::release_emit::{
    emit_branch_release, emit_release, future_readiness_for_state, param_capability,
    record_future_readiness, transfer_future_readiness, NoMacroFragments,
};
use sla_codegen_sab::release_meta::mark_non_owning_reg;
use sla_codegen_sab::release_prim::{emit_stack_alloc, is_released};
use sla_codegen_sab::state::CodegenState;
use sla_codegen_sab::types::{FutureReadiness, MutexGuardValue};
use sla_lowering::MapCheckInfo;


    fn kinds(state: CodegenState) -> Vec<InstKind> {
        state.builder.finish().instructions.iter().map(|i| i.kind).collect()
    }

    #[test]
    fn plain_release_emits_once_and_marks() {
        let mut st = CodegenState::new();
        let r = st.builder.intern("x");
        let mut frags = NoMacroFragments;
        emit_release(&mut st, &mut frags, r).unwrap();
        emit_release(&mut st, &mut frags, r).unwrap();
        assert!(is_released(&st, r));
        assert_eq!(kinds(st), vec![InstKind::Release]);
    }

    #[test]
    fn non_owning_releases_as_move() {
        let mut st = CodegenState::new();
        let r = st.builder.intern("v");
        mark_non_owning_reg(&mut st, r);
        let mut frags = NoMacroFragments;
        emit_release(&mut st, &mut frags, r).unwrap();
        assert_eq!(kinds(st), vec![InstKind::Move]);
    }

    #[test]
    fn stack_alloc_release_is_bookkeeping_only() {
        let mut st = CodegenState::new();
        let r = st.builder.intern("s");
        emit_stack_alloc(&mut st, r, 8);
        emit_stack_alloc(&mut st, r, 8);
        let mut frags = NoMacroFragments;
        emit_release(&mut st, &mut frags, r).unwrap();
        assert!(is_released(&st, r));
        assert_eq!(kinds(st), vec![InstKind::StackAlloc]);
    }

    #[test]
    fn guard_release_without_fragments_is_loud() {
        let mut st = CodegenState::new();
        let g = st.builder.intern("g");
        let m = st.builder.intern("m");
        st.frame.mutex_guard_values.insert(g, MutexGuardValue { mutex_reg: m });
        let mut frags = NoMacroFragments;
        match emit_release(&mut st, &mut frags, g) {
            Err(Error::UnsupportedSabDirectFeature(msg)) => {
                assert!(msg.contains("MUTEX_UNLOCK"), "{msg}");
            }
            other => panic!("expected loud fragment error, got {other:?}"),
        }
    }

    #[test]
    fn readiness_roundtrip_and_unknown_erases() {
        let mut st = CodegenState::new();
        assert_eq!(future_readiness_for_state(&st, 5), FutureReadiness::Unknown);
        record_future_readiness(&mut st, 5, FutureReadiness::Ready);
        transfer_future_readiness(&mut st, 5, 6);
        assert_eq!(future_readiness_for_state(&st, 6), FutureReadiness::Ready);
        assert_eq!(future_readiness_for_state(&st, 5), FutureReadiness::Unknown);
        record_future_readiness(&mut st, 6, FutureReadiness::Unknown);
        assert!(!st.frame.future_readiness.contains_key(&6));
    }

    #[test]
    fn branch_release_has_no_bookkeeping() {
        let mut st = CodegenState::new();
        let r = st.builder.intern("b");
        emit_branch_release(&mut st, r);
        assert!(!is_released(&st, r));
        assert_eq!(kinds(st), vec![InstKind::Release]);
    }

    #[test]
    fn param_capability_borrow_param() {
        let check = MapCheckInfo::new();
        let param = Param {
            name: "p".to_string(),
            ty: Box::new(Type::Borrow(Box::new(Type::Primitive(Primitive::I32)))),
            is_borrow: true,
            is_move: false,
        };
        assert_eq!(param_capability(&check, &param), CapPrefix::Borrow);
    }

