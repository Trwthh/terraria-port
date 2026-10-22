//! Runtime for machine-translated C# (`tools/cs2rs/DESIGN.md`).
//!
//! The model in one sentence: every C# value is a `Copy` Rust value that is valid when all its bytes are zero,
//! and zero means what the CLR means by it (`0`, `false`, `null`, `default(T)`).
//!
//! Nothing here is generated. Keep it small: every function is part of what has to be trusted.

mod arr;
mod collections;
pub mod decimal;
mod exc;
mod gc;
pub mod fmt;
pub mod io;
pub mod iter;
pub mod json;
pub mod lq;
pub mod math;
mod num;
mod object;
pub mod resources;
mod string;
pub mod sys;
pub mod threads;
pub mod time;
mod unported;
mod value;

pub use arr::{fixed_arr, index_from, large_frees, slice_get, slice_len, Arr, Arr2, Arr3};
pub use collections::{get_enumerator, linq, CsKey, NetHash, Dictionary, Enumerable, Enumerator, HashSet, KeyValuePair, List, ListEnum, Queue, Seq, Stack, VecEnum};
pub use exc::{as_exception, cast_exception, exc, exc_init, exc_is, exc_text, rethrow, run, throw, try_, Exception, Flow};
pub use gc::collection_count;
pub use gc::{
    add_root, alloc, alloc_init, field_rescan, gc_begin, gc_phase, gc_profile, gc_profile_report, GcProfile, gc_step, heap_histogram, rescan_histogram, set_verify, wb, wb_slice, wbv, GcBudget, GcPhase, GcStep, dispose_obj, collection_clear, set_conservative_roots, has_conservative_roots, alloc_init_id, allocated_since_collect, as_class, box_id, box_id_or_zero, box_value, cast_class, collect, heap_size, identity_hash, is_boxed,
    is_instance, memberwise_clone, obj_equals, obj_is, obj_reflect, obj_hash_code, obj_to_string, object_equals, new_cell, new_object, register_statics, remove_root, set_poison, tid, type_id_of, unbox, unbox_or_null, Delegate, Gc, GcStats,
    BoxKind, Marker, Nullable, Obj, Object, Rooted, Trace, NativeSlot, NativeVirtuals, native_override, native_register_override, register_scan_block, unregister_scan_block,
};
pub use num::{conv, div, rem, Int};
pub use object::{
    anon_get, anon_new, enum_get_underlying_type, prop_coll_find, type_descriptor_get_properties, type_descriptor_get_properties_of_type, AnonObj, PropColl, PropDesc, as_iface, as_rt, as_rt_if, attribute_get_custom_attribute, delegate_create, obj_get_type, type_get_methods, type_is_enum, MethodInfo, MethodMeta, binding_flags, convert_to_i32, convert_to_i64, member_name, type_get_fields, type_of_meta, FieldInfo, FieldMeta, TypeMeta,
    box_all, member_get_custom_attributes, member_get_custom_attributes_of, meta_of_obj, meta_of_type, meta_register, type_get_custom_attributes, type_get_field, type_info, PropMeta, Reflect, box_enum, enum_tid, obj_to_string_or_empty, box_nullable, boxed_to_string, cast_iface, enum_info, enum_register, type_enum_info, type_of_enum, BoxedText, EnumInfo, cast_rt, cast_rt_if, convert_change_type, enum_to_str, is_iface, obj_as_str, obj_str, str_obj, type_flags, type_full_name, type_name,
    type_generic_definition, type_is_class, type_is_generic, type_is_primitive, type_is_value_type, type_of, type_of_val, unbox_nullable, RtHandle,
};
pub use string::{char_str, concat, legacy_string_hash, str_eq, str_is_null_or_empty, str_switch, to_str, Str, ToStr};
pub use unported::{gap, gaps, stub, stubs_hit, unported, unported_method, unported_new};
pub use value::Value;
/// `System.Math` with the results of the original game (terraria-x87: the x87 instructions and the C runtime routines
/// the 32-bit CLR uses). `tools/cs2rs/native.json` maps the transcendental functions here.
pub use threads::{register_thread_static, yield_point, CctorGuard};
pub use terraria_x87 as x87;

/// Every value type the generated code moves around.
pub trait Val: Copy + 'static + Trace {}
impl<T: Copy + 'static + Trace> Val for T {}

/// `default(T)`: all bytes zero.
///
/// # Safety
/// `T` must be one of the runtime's or the generator's value types (numbers, `bool`, handles, `#[repr(C)]`
/// structs of those). All of them are valid when zeroed; that is the invariant the whole model rests on.
#[inline(always)]
pub unsafe fn zero<T: Copy>() -> T {
    core::mem::zeroed()
}

/// The same value under another Rust type name. Used by the dispatchers of generic library interfaces, where the
/// interface's type parameter and the implementing method's parameter are the same C# type.
///
/// # Safety
/// `A` and `B` must be the same type (checked by size only).
#[inline(always)]
pub unsafe fn retype<A: Copy, B: Copy>(v: A) -> B {
    assert!(core::mem::size_of::<A>() == core::mem::size_of::<B>(), "rt::retype between types of different size");
    core::mem::transmute_copy(&v)
}

/// A point C#'s flow analysis proves unreachable (end of a non-void method, exhausted `Flow`).
#[cold]
#[inline(never)]
pub fn unreachable() -> ! {
    panic!("cs2rs: reached a point the C# compiler proved unreachable")
}

/// Records the simulation thread on first use and checks it afterwards (debug builds).
/// Called by every static initialiser.
#[inline]
pub fn enter() {
    use std::sync::OnceLock;
    use std::thread::ThreadId;
    static OWNER: OnceLock<ThreadId> = OnceLock::new();
    let me = std::thread::current().id();
    let owner = *OWNER.get_or_init(|| me);
    assert!(owner == me, "terraria-gen: generated code was entered from a second thread");
}

/// String literal: `s!("text")` is a `Str` pointing at static data.
#[macro_export]
macro_rules! s {
    ($lit:literal) => {
        $crate::rt::Str::lit($lit)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_is_default() {
        unsafe {
            assert_eq!(zero::<i32>(), 0);
            assert!(!zero::<bool>());
            assert!(zero::<Gc<Object>>().is_null());
            assert!(zero::<Str>().is_null());
            assert!(zero::<Arr<i32>>().is_null());
            assert!(!zero::<Nullable<i32>>().has_value);
        }
    }
}
