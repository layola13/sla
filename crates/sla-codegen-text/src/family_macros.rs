//! Inline helper macro bodies emitted by `generate` when a
//! program needs the HashMap / BTreeMap families (codegen.zig:
//! 5633 `emitHashMapMacros`, 5705 `emitBTreeMapMacros`). Fixed
//! text in the Zig original; kept in their own module so
//! program.rs stays inside the per-file line budget.

/// codegen.zig:5633 `emitHashMapMacros` (fixed text).
pub(crate) const HASHMAP_HELPER_MACROS: &str = "
[MACRO] SLA_MAP_TRY_GET_OPTION %out_opt, %map_reg, %key_reg
    EXPAND MAP_TRY_GET __sla_map_get_ok_%out_opt, __sla_map_get_ptr_%out_opt, %map_reg, %key_reg
    br __sla_map_get_ok_%out_opt -> L_SLA_MAP_GET_SOME_%out_opt, L_SLA_MAP_GET_NONE_%out_opt
L_SLA_MAP_GET_SOME_%out_opt:
    EXPAND OPTION_NEW_SOME %out_opt, __sla_map_get_ptr_%out_opt
    !__sla_map_get_ptr_%out_opt
    !__sla_map_get_ok_%out_opt
    jmp L_SLA_MAP_GET_END_%out_opt
L_SLA_MAP_GET_NONE_%out_opt:
    EXPAND OPTION_NEW_NONE %out_opt
    !__sla_map_get_ptr_%out_opt
    !__sla_map_get_ok_%out_opt
L_SLA_MAP_GET_END_%out_opt:
[END_MACRO]

[MACRO] SLA_MAP_INSERT_OPTION_U64 %out_opt, %map_reg, %key_reg, %value_reg
    EXPAND MAP_INSERT __sla_map_insert_replaced_%out_opt, __sla_map_insert_old_ptr_%out_opt, %map_reg, %key_reg, %value_reg
    br __sla_map_insert_replaced_%out_opt -> L_SLA_MAP_INSERT_SOME_%out_opt, L_SLA_MAP_INSERT_NONE_%out_opt
L_SLA_MAP_INSERT_SOME_%out_opt:
    __sla_map_insert_old_value_%out_opt = load __sla_map_insert_old_ptr_%out_opt+0 as u64
    EXPAND OPTION_NEW_SOME %out_opt, __sla_map_insert_old_value_%out_opt
    !__sla_map_insert_old_value_%out_opt
    !__sla_map_insert_old_ptr_%out_opt
    !__sla_map_insert_replaced_%out_opt
    jmp L_SLA_MAP_INSERT_END_%out_opt
L_SLA_MAP_INSERT_NONE_%out_opt:
    EXPAND OPTION_NEW_NONE %out_opt
    !__sla_map_insert_old_ptr_%out_opt
    !__sla_map_insert_replaced_%out_opt
L_SLA_MAP_INSERT_END_%out_opt:
[END_MACRO]

[MACRO] SLA_MAP_TRY_GET_STR_OPTION %out_opt, %map_reg, %key_reg
    EXPAND MAP_TRY_GET_STR __sla_map_get_str_ok_%out_opt, __sla_map_get_str_ptr_%out_opt, %map_reg, %key_reg
    br __sla_map_get_str_ok_%out_opt -> L_SLA_MAP_GET_STR_SOME_%out_opt, L_SLA_MAP_GET_STR_NONE_%out_opt
L_SLA_MAP_GET_STR_SOME_%out_opt:
    EXPAND OPTION_NEW_SOME %out_opt, __sla_map_get_str_ptr_%out_opt
    !__sla_map_get_str_ptr_%out_opt
    !__sla_map_get_str_ok_%out_opt
    jmp L_SLA_MAP_GET_STR_END_%out_opt
L_SLA_MAP_GET_STR_NONE_%out_opt:
    EXPAND OPTION_NEW_NONE %out_opt
    !__sla_map_get_str_ptr_%out_opt
    !__sla_map_get_str_ok_%out_opt
L_SLA_MAP_GET_STR_END_%out_opt:
[END_MACRO]

[MACRO] SLA_MAP_INSERT_STR_OPTION_U64 %out_opt, %map_reg, %key_reg, %value_reg
    EXPAND MAP_INSERT_STR __sla_map_insert_str_replaced_%out_opt, __sla_map_insert_str_old_ptr_%out_opt, %map_reg, %key_reg, %value_reg
    br __sla_map_insert_str_replaced_%out_opt -> L_SLA_MAP_INSERT_STR_SOME_%out_opt, L_SLA_MAP_INSERT_STR_NONE_%out_opt
L_SLA_MAP_INSERT_STR_SOME_%out_opt:
    __sla_map_insert_str_old_value_%out_opt = load __sla_map_insert_str_old_ptr_%out_opt+0 as u64
    EXPAND OPTION_NEW_SOME %out_opt, __sla_map_insert_str_old_value_%out_opt
    !__sla_map_insert_str_old_value_%out_opt
    !__sla_map_insert_str_old_ptr_%out_opt
    !__sla_map_insert_str_replaced_%out_opt
    jmp L_SLA_MAP_INSERT_STR_END_%out_opt
L_SLA_MAP_INSERT_STR_NONE_%out_opt:
    EXPAND OPTION_NEW_NONE %out_opt
    !__sla_map_insert_str_old_ptr_%out_opt
    !__sla_map_insert_str_replaced_%out_opt
L_SLA_MAP_INSERT_STR_END_%out_opt:
[END_MACRO]
";

/// codegen.zig:5705 `emitBTreeMapMacros` (fixed text).
pub(crate) const BTREEMAP_HELPER_MACROS: &str = "
[MACRO] SLA_BTREE_MAP_INSERT_OPTION_U64 %out_opt, %map_reg, %key_reg, %value_reg
    EXPAND BTREE_MAP_INSERT_OLD __sla_btree_insert_replaced_%out_opt, __sla_btree_insert_old_%out_opt, %map_reg, %key_reg, %value_reg
    br __sla_btree_insert_replaced_%out_opt -> L_SLA_BTREE_INSERT_SOME_%out_opt, L_SLA_BTREE_INSERT_NONE_%out_opt
L_SLA_BTREE_INSERT_SOME_%out_opt:
    EXPAND OPTION_NEW_SOME %out_opt, __sla_btree_insert_old_%out_opt
    !__sla_btree_insert_old_%out_opt
    !__sla_btree_insert_replaced_%out_opt
    jmp L_SLA_BTREE_INSERT_END_%out_opt
L_SLA_BTREE_INSERT_NONE_%out_opt:
    EXPAND OPTION_NEW_NONE %out_opt
    !__sla_btree_insert_old_%out_opt
    !__sla_btree_insert_replaced_%out_opt
L_SLA_BTREE_INSERT_END_%out_opt:
[END_MACRO]

[MACRO] SLA_BTREE_MAP_TRY_GET_OPTION %out_opt, %map_reg, %key_reg
    EXPAND BTREE_MAP_TRY_GET __sla_btree_get_ok_%out_opt, __sla_btree_get_value_%out_opt, %map_reg, %key_reg
    br __sla_btree_get_ok_%out_opt -> L_SLA_BTREE_GET_SOME_%out_opt, L_SLA_BTREE_GET_NONE_%out_opt
L_SLA_BTREE_GET_SOME_%out_opt:
    __sla_btree_get_slot_%out_opt = stack_alloc 8
    store __sla_btree_get_slot_%out_opt+0, __sla_btree_get_value_%out_opt as u64
    EXPAND OPTION_NEW_SOME %out_opt, __sla_btree_get_slot_%out_opt
    !__sla_btree_get_value_%out_opt
    !__sla_btree_get_ok_%out_opt
    jmp L_SLA_BTREE_GET_END_%out_opt
L_SLA_BTREE_GET_NONE_%out_opt:
    EXPAND OPTION_NEW_NONE %out_opt
    !__sla_btree_get_value_%out_opt
    !__sla_btree_get_ok_%out_opt
L_SLA_BTREE_GET_END_%out_opt:
[END_MACRO]
";
