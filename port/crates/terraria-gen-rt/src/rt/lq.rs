//! `System.Linq.Enumerable`: the operators beyond `rt::linq` that the whole game uses. Semantics of .NET
//! Framework 4: which exception for an empty source, deferred execution for the operators that return a sequence
//! (the source is enumerated when the result is), `Sum` of integers checked.

use super::collections::{CsKey, Dictionary, Enumerable, Enumerator, KeyValuePair, Seq, VecEnum};
use super::exc::{exc, throw};
use super::gc::{Delegate, Gc, Obj, Trace};
use super::object::Reflect;
use super::sys::CsOrd;
use super::{Str, Val};

fn need<T: Val>(source: Seq<T>) -> Seq<T> {
    if source.is_null() {
        throw(exc("System.ArgumentNullException", Str::lit("source")))
    }
    source
}
fn no_elements() -> ! {
    throw(exc("System.InvalidOperationException", Str::lit("Sequence contains no elements")))
}
fn no_match() -> ! {
    throw(exc("System.InvalidOperationException", Str::lit("Sequence contains no matching element")))
}
fn zero<T: Val>() -> T {
    // SAFETY: zero is `default(T)` for every value type of the runtime.
    unsafe { core::mem::zeroed() }
}
/// A sequence whose elements are computed from the source each time it is enumerated.
fn deferred<T: Val, R: Val>(source: Seq<T>, make: impl Fn(Vec<T>) -> Vec<R> + 'static, extra: impl Fn(&mut super::Marker) + 'static) -> Seq<R> {
    let source = need(source);
    Seq::lazy(move || Box::new(VecEnum::new(make(source.to_vec()))), move |m| {
        // SAFETY: handles of the runtime.
        unsafe { source.trace(m) };
        extra(m)
    })
}

/// `source.All(predicate)`.
pub fn all<T: Val>(source: Seq<T>, predicate: Delegate<dyn Fn(T) -> bool>) -> bool {
    let mut e = need(source).enumerator();
    while e.move_next() {
        let x = e.current();
        if !predicate.invoke(|f| f(x)) {
            return false;
        }
    }
    true
}

/// `source.Count(predicate)`.
pub fn count_if<T: Val>(source: Seq<T>, predicate: Delegate<dyn Fn(T) -> bool>) -> i32 {
    let mut n = 0i32;
    let mut e = need(source).enumerator();
    while e.move_next() {
        let x = e.current();
        if predicate.invoke(|f| f(x)) {
            n += 1;
        }
    }
    n
}

/// `source.First(predicate)`.
pub fn first_if<T: Val>(source: Seq<T>, predicate: Delegate<dyn Fn(T) -> bool>) -> T {
    let mut e = need(source).enumerator();
    while e.move_next() {
        let x = e.current();
        if predicate.invoke(|f| f(x)) {
            return x;
        }
    }
    no_match()
}

/// `source.FirstOrDefault()`.
pub fn first_or_default<T: Val>(source: Seq<T>) -> T {
    let mut e = need(source).enumerator();
    if e.move_next() {
        e.current()
    } else {
        zero()
    }
}

/// `source.LastOrDefault()`.
pub fn last_or_default<T: Val>(source: Seq<T>) -> T {
    need(source).to_vec().last().copied().unwrap_or_else(zero)
}

/// `source.Single(predicate)`.
pub fn single_if<T: Val>(source: Seq<T>, predicate: Delegate<dyn Fn(T) -> bool>) -> T {
    let mut found: Option<T> = None;
    let mut e = need(source).enumerator();
    while e.move_next() {
        let x = e.current();
        if predicate.invoke(|f| f(x)) {
            if found.is_some() {
                throw(exc("System.InvalidOperationException", Str::lit("Sequence contains more than one matching element")))
            }
            found = Some(x);
        }
    }
    found.unwrap_or_else(|| no_match())
}

/// `source.SingleOrDefault()`.
pub fn single_or_default<T: Val>(source: Seq<T>) -> T {
    let v = need(source).to_vec();
    match v.len() {
        0 => zero(),
        1 => v[0],
        _ => throw(exc("System.InvalidOperationException", Str::lit("Sequence contains more than one element"))),
    }
}

/// `source.ElementAt(index)`.
pub fn element_at<T: Val>(source: Seq<T>, index: i32) -> T {
    let v = need(source).to_vec();
    if index < 0 || index as usize >= v.len() {
        throw(exc("System.ArgumentOutOfRangeException", Str::lit("index")))
    }
    v[index as usize]
}

/// `source.Select((element, index) => ..)`. Deferred to the enumeration, where the whole source is projected at once
/// (deviation, as for the other operators built on `deferred`: .NET projects element by element).
pub fn select_indexed<T: Val, R: Val>(source: Seq<T>, selector: Delegate<dyn Fn(T, i32) -> R>) -> Seq<R> {
    deferred(source, move |v| v.into_iter().enumerate().map(|(i, x)| selector.invoke(|f| f(x, i as i32))).collect(), move |m| unsafe { selector.trace(m) })
}
/// `source.Where((element, index) => ..)`.
pub fn where_indexed<T: Val>(source: Seq<T>, predicate: Delegate<dyn Fn(T, i32) -> bool>) -> Seq<T> {
    deferred(source, move |v| v.into_iter().enumerate().filter(|(i, x)| predicate.invoke(|f| f(*x, *i as i32))).map(|(_, x)| x).collect(), move |m| unsafe { predicate.trace(m) })
}
/// `source.Skip(count)`.
pub fn skip<T: Val>(source: Seq<T>, count: i32) -> Seq<T> {
    deferred(source, move |v| v.into_iter().skip(count.max(0) as usize).collect(), |_| {})
}
/// `source.Take(count)`.
pub fn take<T: Val>(source: Seq<T>, count: i32) -> Seq<T> {
    deferred(source, move |v| v.into_iter().take(count.max(0) as usize).collect(), |_| {})
}
/// `source.TakeWhile(predicate)`.
pub fn take_while<T: Val>(source: Seq<T>, predicate: Delegate<dyn Fn(T) -> bool>) -> Seq<T> {
    deferred(source, move |v| v.into_iter().take_while(|x| predicate.invoke(|f| f(*x))).collect(), move |m| unsafe { predicate.trace(m) })
}
/// `source.Reverse()`.
pub fn reverse<T: Val>(source: Seq<T>) -> Seq<T> {
    deferred(source, |mut v| {
        v.reverse();
        v
    }, |_| {})
}

/// `first.Except(second)`: the distinct elements of `first` that are not in `second`, in the order of `first`.
pub fn except<T: CsKey>(first: Seq<T>, second: Seq<T>) -> Seq<T> {
    let second = need(second);
    deferred(
        first,
        move |v| {
            let mut seen: Vec<T> = second.to_vec();
            let mut out = Vec::new();
            for x in v {
                if !seen.iter().any(|y| y.cs_eq(&x)) {
                    seen.push(x);
                    out.push(x);
                }
            }
            out
        },
        move |m| unsafe { second.trace(m) },
    )
}

/// `first.Intersect(second)`: the distinct elements of `first` that are also in `second`, in the order of `first`.
pub fn intersect<T: CsKey>(first: Seq<T>, second: Seq<T>) -> Seq<T> {
    let second = need(second);
    deferred(
        first,
        move |v| {
            let mut rest: Vec<T> = second.to_vec();
            let mut out = Vec::new();
            for x in v {
                if let Some(p) = rest.iter().position(|y| y.cs_eq(&x)) {
                    // Set.Remove: every equal element leaves the set, so a second occurrence in `first` is not yielded
                    rest.retain(|y| !y.cs_eq(&x));
                    let _ = p;
                    out.push(x);
                }
            }
            out
        },
        move |m| unsafe { second.trace(m) },
    )
}

/// `first.Zip(second, resultSelector)`.
pub fn zip<A: Val, B: Val, R: Val>(first: Seq<A>, second: Seq<B>, selector: Delegate<dyn Fn(A, B) -> R>) -> Seq<R> {
    let second = need(second);
    deferred(first, move |v| v.into_iter().zip(second.to_vec()).map(|(a, b)| selector.invoke(|f| f(a, b))).collect(), move |m| unsafe {
        second.trace(m);
        selector.trace(m)
    })
}

fn checked_sum_i32(v: impl Iterator<Item = i32>) -> i32 {
    let mut s = 0i32;
    for x in v {
        s = s.checked_add(x).unwrap_or_else(|| throw(exc("System.OverflowException", Str::lit("Arithmetic operation resulted in an overflow."))));
    }
    s
}
/// `source.Sum()` for `int`: checked.
pub fn sum_i32(source: Seq<i32>) -> i32 {
    checked_sum_i32(need(source).to_vec().into_iter())
}
/// `source.Sum()` for `long`: checked.
pub fn sum_i64(source: Seq<i64>) -> i64 {
    let mut s = 0i64;
    for x in need(source).to_vec() {
        s = s.checked_add(x).unwrap_or_else(|| throw(exc("System.OverflowException", Str::lit("Arithmetic operation resulted in an overflow."))));
    }
    s
}
/// `source.Sum(selector)` for `int`.
pub fn sum_by_i32<T: Val>(source: Seq<T>, selector: Delegate<dyn Fn(T) -> i32>) -> i32 {
    checked_sum_i32(need(source).to_vec().into_iter().map(|x| selector.invoke(|f| f(x))))
}
/// `source.Sum(selector)` for `double`.
pub fn sum_by_f64<T: Val>(source: Seq<T>, selector: Delegate<dyn Fn(T) -> f64>) -> f64 {
    let mut s = 0.0f64;
    for x in need(source).to_vec() {
        s += selector.invoke(|f| f(x));
    }
    s
}
/// `source.Average(selector)` for `double`: sum / count, `InvalidOperationException` for an empty source.
pub fn average_by_f64<T: Val>(source: Seq<T>, selector: Delegate<dyn Fn(T) -> f64>) -> f64 {
    let v = need(source).to_vec();
    if v.is_empty() {
        no_elements()
    }
    let mut s = 0.0f64;
    for x in &v {
        s += selector.invoke(|f| f(*x));
    }
    s / v.len() as f64
}

/// `source.Max()` for `int`.
pub fn max_i32(source: Seq<i32>) -> i32 {
    need(source).to_vec().into_iter().max().unwrap_or_else(|| no_elements())
}
/// `source.Max()` for `double`: NaN is smaller than every number (as `Enumerable.Max` treats it).
pub fn max_f64(source: Seq<f64>) -> f64 {
    let v = need(source).to_vec();
    if v.is_empty() {
        no_elements()
    }
    let mut m = v[0];
    for x in &v[1..] {
        if *x > m || m.is_nan() {
            m = *x;
        }
    }
    m
}
/// `source.Max()` through `Comparer<T>.Default`; a reference-type source with no elements gives null.
pub fn max<T: CsOrd>(source: Seq<T>) -> T {
    let v = need(source).to_vec();
    if v.is_empty() {
        if matches!(T::BOX, super::BoxKind::Value(_)) {
            no_elements()
        }
        return zero();
    }
    let mut m = v[0];
    for x in &v[1..] {
        if x.cs_cmp(&m) > 0 {
            m = *x;
        }
    }
    m
}
/// `source.Max(selector)` for `int`.
pub fn max_by_i32<T: Val>(source: Seq<T>, selector: Delegate<dyn Fn(T) -> i32>) -> i32 {
    need(source).to_vec().into_iter().map(|x| selector.invoke(|f| f(x))).max().unwrap_or_else(|| no_elements())
}
/// `source.Min(selector)` for `int`.
pub fn min_by_i32<T: Val>(source: Seq<T>, selector: Delegate<dyn Fn(T) -> i32>) -> i32 {
    need(source).to_vec().into_iter().map(|x| selector.invoke(|f| f(x))).min().unwrap_or_else(|| no_elements())
}

/// `source.ToDictionary(keySelector)`: `ArgumentException` on a duplicate key (from `Dictionary.Add`).
pub fn to_dictionary<T: Val, K: CsKey>(source: Seq<T>, key: Delegate<dyn Fn(T) -> K>) -> Gc<Dictionary<K, T>> {
    let d = Dictionary::<K, T>::new_gc();
    for x in need(source).to_vec() {
        let k = key.invoke(|f| f(x));
        // SAFETY: fresh, non-null dictionary; the reference ends with the statement.
        unsafe { (*d.ptr()).add(k, x) };
    }
    d
}

/// `source.Cast<T>()` for a source that is only known as `IEnumerable` (an array or a collection held as `object`):
/// each element is unboxed / downcast as `(T)x` when the result is enumerated.
pub fn cast<T: Val>(source: Obj) -> Seq<T> {
    if source.is_null() {
        throw(exc("System.ArgumentNullException", Str::lit("source")))
    }
    Seq::lazy(
        move || {
            let items = match super::obj_reflect(source) {
                Reflect::Seq { items, .. } | Reflect::Multi { items, .. } => items,
                _ => crate::host::unimplemented_value("System.Linq.Enumerable.Cast<TResult>(IEnumerable) over an object that is not a runtime collection"),
            };
            // SAFETY: `from_obj` checks boxed values; handles are retyped as the C# cast would.
            Box::new(VecEnum::new(items.into_iter().map(|o| unsafe { T::from_obj(o) }).collect()))
        },
        move |m| m.mark(source),
    )
}

#[allow(dead_code)]
fn _kv<K: Val, V: Val>(_: KeyValuePair<K, V>) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rt::{try_, Arr};

    fn seq(v: &[i32]) -> Seq<i32> {
        Seq::from_arr(Arr::from_slice(v))
    }
    fn d<A: 'static, R: 'static>(f: impl Fn(A) -> R + 'static) -> Delegate<dyn Fn(A) -> R> {
        Delegate::new(Box::new(f))
    }

    #[test]
    fn scalar_operators() {
        assert!(all(seq(&[2, 4]), d(|x: i32| x % 2 == 0)) && !all(seq(&[2, 3]), d(|x: i32| x % 2 == 0)) && all(seq(&[]), d(|_: i32| false)));
        assert_eq!(count_if(seq(&[1, 2, 3, 4]), d(|x: i32| x > 2)), 2);
        assert_eq!(first_if(seq(&[1, 2, 3]), d(|x: i32| x > 1)), 2);
        assert!(try_(|| first_if(seq(&[1]), d(|x: i32| x > 1))).is_err());
        assert_eq!(first_or_default(seq(&[])), 0);
        assert_eq!(last_or_default(seq(&[5, 6])), 6);
        assert_eq!(single_or_default(seq(&[])), 0);
        assert_eq!(single_or_default(seq(&[9])), 9);
        assert!(try_(|| single_or_default(seq(&[1, 2]))).is_err());
        assert_eq!(single_if(seq(&[1, 2, 3]), d(|x: i32| x == 3)), 3);
        assert!(try_(|| single_if(seq(&[3, 3]), d(|x: i32| x == 3))).is_err());
        assert_eq!(element_at(seq(&[7, 8]), 1), 8);
        assert!(try_(|| element_at(seq(&[7, 8]), 2)).is_err());
    }

    #[test]
    fn sequence_operators_are_deferred_and_ordered() {
        assert_eq!(skip(seq(&[1, 2, 3]), 1).to_vec(), [2, 3]);
        assert_eq!(skip(seq(&[1, 2, 3]), -5).to_vec(), [1, 2, 3]);
        assert_eq!(take(seq(&[1, 2, 3]), 2).to_vec(), [1, 2]);
        assert_eq!(take_while(seq(&[1, 2, 5, 1]), d(|x: i32| x < 3)).to_vec(), [1, 2]);
        assert_eq!(reverse(seq(&[1, 2, 3])).to_vec(), [3, 2, 1]);
        assert_eq!(except(seq(&[1, 2, 2, 3, 1]), seq(&[2])).to_vec(), [1, 3]);
        assert_eq!(intersect(seq(&[3, 1, 3, 2]), seq(&[3, 2, 9])).to_vec(), [3, 2]);
        let z = zip(seq(&[1, 2, 3]), seq(&[10, 20]), Delegate::new(Box::new(|a: i32, b: i32| a + b)));
        assert_eq!(z.to_vec(), [11, 22]);
        // deferred: the source array is read when the result is enumerated
        let a = Arr::from_slice(&[1, 2, 3]);
        let s = skip(Seq::from_arr(a), 1);
        unsafe { *a.at(2) = 30 };
        assert_eq!(s.to_vec(), [2, 30]);
    }

    #[test]
    fn aggregates() {
        assert_eq!(sum_i32(seq(&[1, 2, 3])), 6);
        assert!(try_(|| sum_i32(seq(&[i32::MAX, 1]))).is_err());
        assert_eq!(sum_by_i32(seq(&[1, 2]), d(|x: i32| x * 10)), 30);
        assert_eq!(sum_by_f64(seq(&[1, 2]), d(|x: i32| x as f64 / 2.0)), 1.5);
        assert_eq!(average_by_f64(seq(&[1, 2]), d(|x: i32| x as f64)), 1.5);
        assert!(try_(|| average_by_f64(seq(&[]), d(|x: i32| x as f64))).is_err());
        assert_eq!(max_i32(seq(&[3, 9, 4])), 9);
        assert!(try_(|| max_i32(seq(&[]))).is_err());
        assert_eq!(max(seq(&[3, 9, 4])), 9);
        assert_eq!(max_by_i32(seq(&[3, 9, 4]), d(|x: i32| -x)), -3);
        assert_eq!(min_by_i32(seq(&[3, 9, 4]), d(|x: i32| -x)), -9);
        assert_eq!(max_f64(Seq::from_arr(Arr::from_slice(&[1.0, f64::NAN, 0.5]))), 1.0);
        let dict = to_dictionary(seq(&[1, 2]), d(|x: i32| x * 2));
        unsafe {
            assert_eq!((*dict.ptr()).get_item(4), 2);
        }
        assert!(try_(|| to_dictionary(seq(&[1, 1]), d(|x: i32| x))).is_err());
    }

    #[test]
    fn cast_reads_a_collection_held_as_object() {
        let boxed: Vec<Obj> = [1i32, 2, 3].iter().map(|x| crate::rt::box_value(*x, crate::rt::tid::I32)).collect();
        let arr: Arr<Obj> = Arr::from_slice(&boxed);
        assert_eq!(cast::<i32>(arr.up()).to_vec(), [1, 2, 3]);
    }
}
