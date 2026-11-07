//! The SAB string pool: construction order is part of the format.
//!
//! Port of the pool-building half of sci `sab.zig`
//! `encodeProgramWithConsts` (lines 921-986) and `addPoolText`
//! (lines 131-144). The pool is an insertion-ordered set: the first
//! occurrence of a string fixes its id, later occurrences reuse it.
//!
//! Collection order (must not change — pool ids are baked into every
//! serialized operand, name and text):
//! 1. the module's symbol table, in order;
//! 2. per instruction, in order: `atomic_expected_text`,
//!    `atomic_new_text`, each of `native_reg_names`,
//!    `package_identity`, `upstream_loc.file`, then each operand's
//!    `Text` / `NativeText` payload in operand order;
//! 3. per function sig, in order: `name`, `upstream_file`,
//!    `upstream_loc.file`, `llvm_name`, then each param's name;
//! 4. per const decl, in order: `name`, `literal_text`,
//!    `upstream_loc.file`, then the strings inside the const value
//!    (struct field names recursively, vtable slot name + func name).

use std::collections::HashMap;

use crate::ir::{ConstValue, Module, Operand};

/// Insertion-ordered string set assigning pool ids.
#[derive(Debug, Default)]
pub struct StringPool {
    entries: Vec<String>,
    index: HashMap<String, u32>,
}

impl StringPool {
    pub fn new() -> Self {
        StringPool::default()
    }

    /// Add `text` if unseen and return its pool id
    /// (Zig `addPoolText`).
    pub fn add(&mut self, text: &str) -> u32 {
        if let Some(&id) = self.index.get(text) {
            return id;
        }
        let id = self.entries.len() as u32;
        self.entries.push(text.to_string());
        self.index.insert(text.to_string(), id);
        id
    }

    /// Look up a string that pool construction must have added.
    pub fn id_of(&self, text: &str) -> Option<u32> {
        self.index.get(text).copied()
    }

    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn add_const_value_symbols(pool: &mut StringPool, value: &ConstValue) {
    match value {
        ConstValue::Struct(fields) => {
            for field in fields {
                pool.add(&field.name);
                add_const_value_symbols(pool, &field.value);
            }
        }
        ConstValue::VTable(slots) => {
            for slot in slots {
                pool.add(&slot.name);
                pool.add(&slot.func_name);
            }
        }
        ConstValue::Hex(_) | ConstValue::Utf8(_) | ConstValue::Repeat(_) => {}
    }
}

/// Build the pool for `module` in exactly the Zig encoder's order.
pub fn build_pool(module: &Module) -> StringPool {
    let mut pool = StringPool::new();
    for symbol in &module.symbols {
        pool.add(symbol);
    }
    for inst in &module.instructions {
        if let Some(text) = &inst.atomic_expected_text {
            pool.add(text);
        }
        if let Some(text) = &inst.atomic_new_text {
            pool.add(text);
        }
        for name in &inst.native_reg_names {
            pool.add(name);
        }
        if let Some(identity) = &inst.package_identity {
            pool.add(identity);
        }
        if let Some(loc) = &inst.upstream_loc {
            pool.add(&loc.file);
        }
        for operand in &inst.operands {
            match operand {
                Operand::Text(text) | Operand::NativeText(text) => {
                    pool.add(text);
                }
                _ => {}
            }
        }
    }
    for sig in &module.function_sigs {
        pool.add(&sig.name);
        if let Some(file) = &sig.upstream_file {
            pool.add(file);
        }
        if let Some(loc) = &sig.upstream_loc {
            pool.add(&loc.file);
        }
        if let Some(name) = &sig.llvm_name {
            pool.add(name);
        }
        for param in &sig.params {
            pool.add(&param.name);
        }
    }
    for decl in &module.const_decls {
        pool.add(&decl.name);
        pool.add(&decl.literal_text);
        if let Some(loc) = &decl.upstream_loc {
            pool.add(&loc.file);
        }
        add_const_value_symbols(&mut pool, &decl.value);
    }
    pool
}
