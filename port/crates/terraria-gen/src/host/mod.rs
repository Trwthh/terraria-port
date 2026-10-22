//! Hand-written providers for C# members that the translator config maps natively and that depend on game
//! content outside the translated code. Each one names the original it stands in for.
//!
//! The data is installed by the host (the item test loads it from `refdata/`, which was dumped from the real
//! game). A provider that was not given its data panics: a missing table must never look like an empty one.

use crate::rt::{Rooted, Str};
use std::cell::RefCell;

pub use crate::rt::host::{as_instance, cast_instance, configured, inert, is_instance, new_opaque, reached, unimplemented, unimplemented_value};

/// Generated: one function per library member the translated code calls (`tools/cs2rs/HOST_API.md`).
pub mod api;

/// `Terraria.Localization.LocalizedText` as the translated code sees it: a key and its text. Objects are made by
/// [`language_get_text`] and live for ever (the original keeps them in `LanguageManager` for the whole run too).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LocalizedText {
    pub key: Str,
    pub value: Str,
    /// `HasValue`: a text was found (the original compares the English value with the key).
    pub has_value: bool,
}
unsafe impl crate::rt::Trace for LocalizedText {
    unsafe fn trace(&self, m: &mut crate::rt::Marker) {
        self.key.trace(m);
        self.value.trace(m);
    }
}

thread_local! {
    static TEXTS: RefCell<std::collections::HashMap<String, Rooted<crate::rt::Gc<LocalizedText>>>> = RefCell::new(std::collections::HashMap::new());
    static TEXTS_INSTALLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static NPC_NAMES: RefCell<std::collections::HashMap<i32, Str>> = RefCell::new(std::collections::HashMap::new());
    static ITEM_NAMES: RefCell<Vec<Str>> = const { RefCell::new(Vec::new()) };
}

/// Installs the item name table: `names[id]` is `Lang.GetItemNameValue(id)`.
pub fn set_item_names(names: &[Option<String>]) {
    let v: Vec<Str> = names.iter().map(|n| n.as_deref().map(Str::new_static).unwrap_or(Str::NULL)).collect();
    ITEM_NAMES.with(|t| *t.borrow_mut() = v);
}

/// `Lang.GetItemNameValue(id)` (Lang.cs:220) = `GetItemName(id).Value` (Lang.cs:198-209). The name cache is
/// filled from the localisation files through `LanguageManager`, which is not translated; the host supplies it.
///
/// As in the original, an id outside `1..ItemID.Count` or without a cached name gives `LocalizedText.Empty`,
/// whose value is `""`. Negative ids go through `ItemID.FromNetId` in the original (Lang.cs:200-203); that
/// branch is not provided here and panics instead of guessing.
pub fn lang_get_item_name_value(id: i32) -> Str {
    assert!(id >= 0, "host::lang_get_item_name_value: negative net ids (ItemID.FromNetId) are not provided");
    ITEM_NAMES.with(|t| {
        let t = t.borrow();
        assert!(!t.is_empty(), "host::set_item_names was not called");
        match t.get(id as usize) {
            Some(name) if id > 0 && !name.is_null() => *name,
            _ => Str::lit(""),
        }
    })
}

fn text_object(key: &str, value: &str, has_value: bool) -> Rooted<crate::rt::Gc<LocalizedText>> {
    Rooted::new(crate::rt::alloc_init(LocalizedText { key: Str::new_static(key), value: Str::new_static(value), has_value }))
}

/// Installs localisation texts: (`Category.Key`, text) pairs, what `LanguageManager` loads from the game's
/// `Terraria.Localization.Content.<culture>.*.json` resources. May be called several times.
pub fn install_texts(texts: &[(String, String)]) {
    TEXTS.with(|t| {
        let mut t = t.borrow_mut();
        for (k, v) in texts {
            t.insert(k.clone(), text_object(k, v, true));
        }
    });
    TEXTS_INSTALLED.with(|f| f.set(true));
}

/// `Language.GetText(key)` (Language.cs:10 -> LanguageManager.GetText): the text object of a key; a key without
/// a text gives an object whose value is the key itself, as in the original. Panics when no texts were
/// installed: a missing table must not look like an empty one.
pub fn language_get_text(key: Str) -> crate::rt::Gc<LocalizedText> {
    assert!(TEXTS_INSTALLED.with(|f| f.get()), "host::install_texts was not called");
    let k = key.as_str().to_owned();
    TEXTS.with(|t| t.borrow_mut().entry(k.clone()).or_insert_with(|| text_object(&k, &k, false)).get())
}

/// `Language.Exists(key)`.
pub fn language_exists(key: Str) -> bool {
    assert!(TEXTS_INSTALLED.with(|f| f.get()), "host::install_texts was not called");
    TEXTS.with(|t| t.borrow().get(key.as_str()).map(|o| unsafe { (*o.get().0).has_value }).unwrap_or(false))
}

/// `LocalizedText.Empty` (LocalizedText.cs:9): key "" and value "".
pub fn language_empty() -> crate::rt::Gc<LocalizedText> {
    TEXTS.with(|t| t.borrow_mut().entry("\u{0}empty".to_owned()).or_insert_with(|| text_object("", "", false)).get())
}

thread_local! {
    static MISC: RefCell<Option<Rooted<crate::rt::Arr<crate::rt::Gc<LocalizedText>>>>> = const { RefCell::new(None) };
}

/// `Lang.misc` (Lang.cs:33, a `LocalizedText[201]`): `Lang.InitializeLegacyLocalization` fills it with
/// `Language.GetText("LegacyMisc." + i)` (`FillNameCacheArray`). Built on first use from the installed texts.
pub fn lang_misc() -> crate::rt::Arr<crate::rt::Gc<LocalizedText>> {
    MISC.with(|m| {
        let mut m = m.borrow_mut();
        if m.is_none() {
            let a = crate::rt::Arr::<crate::rt::Gc<LocalizedText>>::new(201);
            for i in 0..201 {
                // SAFETY: index in range.
                unsafe { *a.at(i) = language_get_text(Str::new(&format!("LegacyMisc.{i}"))) };
            }
            *m = Some(Rooted::new(a));
        }
        m.as_ref().map(|r| r.get()).unwrap()
    })
}

/// Installs the NPC name table: `names[netID]` is `Lang.GetNPCNameValue(netID)`.
pub fn set_npc_names(names: &[(i32, String)]) {
    NPC_NAMES.with(|t| *t.borrow_mut() = names.iter().map(|(id, n)| (*id, Str::new_static(n))).collect());
}

/// `Lang.GetNPCNameValue(netID)` (Lang.cs:175) = `GetNPCName(netID).Value`: the display name from the
/// localisation files (not translated; the host supplies the table). An id without an entry panics: the original
/// returns a `LocalizedText` for every id it knows and the table holds all of them.
pub fn lang_get_npc_name_value(net_id: i32) -> Str {
    NPC_NAMES.with(|t| {
        let t = t.borrow();
        assert!(!t.is_empty(), "host::set_npc_names was not called");
        match t.get(&net_id) {
            Some(n) => *n,
            None => panic!("host::lang_get_npc_name_value: no name for netID {net_id}"),
        }
    })
}

/// `Environment.TickCount`: milliseconds since the process started (the original counts from boot). Only used
/// to seed generators created with `new UnifiedRandom()`.
pub fn environment_tick_count() -> i32 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as i32
}
