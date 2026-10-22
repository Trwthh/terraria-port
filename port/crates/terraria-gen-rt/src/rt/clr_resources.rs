//! The embedded (manifest) resources of a .NET assembly file, read from the file itself (ECMA-335 partition II:
//! the PE file's CLI header, the metadata `#~` tables up to `ManifestResource` (0x28), the resources section). Used
//! by the browser build, which has the player's own `Terraria.exe` and nothing extracted from it; natively it is
//! checked against the decompile's extracted copies (test below).
//!
//! Only what that needs: resources stored in this file (`Implementation` null), in manifest (row) order, as byte
//! ranges of the file.

use std::ops::Range;

/// (name, bytes of the resource in `file`) for every resource embedded in the assembly, in manifest order.
pub fn manifest_resources(file: &[u8]) -> Result<Vec<(String, Range<usize>)>, String> {
    let r = Reader { b: file };
    // PE: DOS header -> "PE\0\0" -> COFF header -> optional header -> data directories and sections
    let pe = r.u32(0x3c)? as usize;
    if r.slice(pe, 4)? != b"PE\0\0" {
        return Err("not a PE file".into());
    }
    let coff = pe + 4;
    let sections = r.u16(coff + 2)? as usize;
    let opt_size = r.u16(coff + 16)? as usize;
    let opt = coff + 20;
    let dirs = match r.u16(opt)? {
        0x10b => opt + 96,
        0x20b => opt + 112,
        m => return Err(format!("unknown optional header magic {m:#x}")),
    };
    let section_table = opt + opt_size;
    let rva = |v: u32| -> Result<usize, String> {
        for s in 0..sections {
            let h = section_table + s * 40;
            let (vsize, vaddr, raw_size, raw_ptr) = (r.u32(h + 8)?, r.u32(h + 12)?, r.u32(h + 16)?, r.u32(h + 20)?);
            if v >= vaddr && v < vaddr + vsize.max(raw_size) {
                return Ok((v - vaddr + raw_ptr) as usize);
            }
        }
        Err(format!("RVA {v:#x} is in no section"))
    };
    let cli = rva(r.u32(dirs + 14 * 8)?)?;
    let metadata = rva(r.u32(cli + 8)?)?;
    let resources_rva = r.u32(cli + 24)?;
    // metadata root -> stream headers
    if r.u32(metadata)? != 0x424a_5342 {
        return Err("no metadata signature".into());
    }
    let version_len = r.u32(metadata + 12)? as usize;
    let mut p = metadata + 16 + version_len + 2;
    let streams = r.u16(p)? as usize;
    p += 2;
    let (mut tables, mut strings) = (None, None);
    for _ in 0..streams {
        let (off, _size) = (r.u32(p)? as usize, r.u32(p + 4)?);
        let mut q = p + 8;
        let start = q;
        while r.u8(q)? != 0 {
            q += 1;
        }
        let name = std::str::from_utf8(r.slice(start, q - start)?).map_err(|e| e.to_string())?;
        p = start + ((q - start + 1 + 3) & !3);
        match name {
            "#~" | "#-" => tables = Some(metadata + off),
            "#Strings" => strings = Some(metadata + off),
            _ => {}
        }
    }
    let (t, strings) = (tables.ok_or("no #~ stream")?, strings.ok_or("no #Strings stream")?);
    let heap_sizes = r.u8(t + 6)?;
    let valid = r.u64(t + 8)?;
    let mut rows = [0u32; 64];
    let mut q = t + 24;
    for (i, row) in rows.iter_mut().enumerate() {
        if valid >> i & 1 == 1 {
            *row = r.u32(q)?;
            q += 4;
        }
    }
    let str_ix = if heap_sizes & 1 != 0 { 4 } else { 2 };
    let guid_ix = if heap_sizes & 2 != 0 { 4 } else { 2 };
    let blob_ix = if heap_sizes & 4 != 0 { 4 } else { 2 };
    let ix = |table: usize| if rows[table] < 0x1_0000 { 2 } else { 4 };
    let coded = |tables: &[usize], bits: u32| if tables.iter().map(|&t| rows[t]).max().unwrap_or(0) < (1 << (16 - bits)) { 2 } else { 4 };
    // coded index kinds (II.24.2.6); 0x40 stands for an unused tag (no rows)
    let type_def_or_ref = coded(&[0x02, 0x01, 0x1b], 2);
    let has_constant = coded(&[0x04, 0x08, 0x17], 2);
    let has_custom_attribute = coded(&[0x06, 0x04, 0x01, 0x02, 0x08, 0x09, 0x0a, 0x00, 0x0e, 0x17, 0x14, 0x11, 0x1a, 0x1b, 0x20, 0x23, 0x26, 0x27, 0x28, 0x2a, 0x2c, 0x2b], 5);
    let has_field_marshal = coded(&[0x04, 0x08], 1);
    let has_decl_security = coded(&[0x02, 0x06, 0x20], 2);
    let member_ref_parent = coded(&[0x02, 0x01, 0x1a, 0x06, 0x1b], 3);
    let has_semantics = coded(&[0x14, 0x17], 1);
    let method_def_or_ref = coded(&[0x06, 0x0a], 1);
    let member_forwarded = coded(&[0x04, 0x06], 1);
    let implementation = coded(&[0x26, 0x23, 0x27], 2);
    let custom_attribute_type = coded(&[0x06, 0x0a], 3);
    let resolution_scope = coded(&[0x00, 0x1a, 0x23, 0x01], 2);
    // row sizes of tables 0x00..0x28 (II.22)
    let size: [usize; 0x29] = [
        2 + str_ix + 3 * guid_ix,                                    // 00 Module
        resolution_scope + 2 * str_ix,                               // 01 TypeRef
        4 + 2 * str_ix + type_def_or_ref + ix(0x04) + ix(0x06),      // 02 TypeDef
        ix(0x04),                                                    // 03 FieldPtr
        2 + str_ix + blob_ix,                                        // 04 Field
        ix(0x06),                                                    // 05 MethodPtr
        4 + 2 + 2 + str_ix + blob_ix + ix(0x08),                     // 06 MethodDef
        ix(0x08),                                                    // 07 ParamPtr
        2 + 2 + str_ix,                                              // 08 Param
        ix(0x02) + type_def_or_ref,                                  // 09 InterfaceImpl
        member_ref_parent + str_ix + blob_ix,                        // 0a MemberRef
        2 + has_constant + blob_ix,                                  // 0b Constant
        has_custom_attribute + custom_attribute_type + blob_ix,      // 0c CustomAttribute
        has_field_marshal + blob_ix,                                 // 0d FieldMarshal
        2 + has_decl_security + blob_ix,                             // 0e DeclSecurity
        2 + 4 + ix(0x02),                                            // 0f ClassLayout
        4 + ix(0x04),                                                // 10 FieldLayout
        blob_ix,                                                     // 11 StandAloneSig
        ix(0x02) + ix(0x14),                                         // 12 EventMap
        ix(0x14),                                                    // 13 EventPtr
        2 + str_ix + type_def_or_ref,                                // 14 Event
        ix(0x02) + ix(0x17),                                         // 15 PropertyMap
        ix(0x17),                                                    // 16 PropertyPtr
        2 + str_ix + blob_ix,                                        // 17 Property
        2 + ix(0x06) + has_semantics,                                // 18 MethodSemantics
        ix(0x02) + 2 * method_def_or_ref,                            // 19 MethodImpl
        str_ix,                                                      // 1a ModuleRef
        blob_ix,                                                     // 1b TypeSpec
        2 + member_forwarded + str_ix + ix(0x1a),                    // 1c ImplMap
        4 + ix(0x04),                                                // 1d FieldRVA
        8,                                                           // 1e EncLog
        4,                                                           // 1f EncMap
        4 + 4 * 2 + 4 + blob_ix + 2 * str_ix,                        // 20 Assembly
        4,                                                           // 21 AssemblyProcessor
        12,                                                          // 22 AssemblyOS
        4 * 2 + 4 + blob_ix + 2 * str_ix + blob_ix,                  // 23 AssemblyRef
        4 + ix(0x23),                                                // 24 AssemblyRefProcessor
        12 + ix(0x23),                                               // 25 AssemblyRefOS
        4 + str_ix + blob_ix,                                        // 26 File
        4 + 4 + 2 * str_ix + implementation,                         // 27 ExportedType
        4 + 4 + str_ix + implementation,                             // 28 ManifestResource
    ];
    let mut at = q;
    for (table, row_size) in size.iter().enumerate().take(0x28) {
        at += rows[table] as usize * row_size;
    }
    let read_ix = |p: usize, n: usize| -> Result<u32, String> { if n == 2 { r.u16(p).map(u32::from) } else { r.u32(p) } };
    let resources = rva(resources_rva).ok();
    let mut out = Vec::new();
    for i in 0..rows[0x28] as usize {
        let row = at + i * size[0x28];
        let offset = r.u32(row)? as usize;
        let name_ix = read_ix(row + 8, str_ix)? as usize;
        let implementation = read_ix(row + 8 + str_ix, implementation)?;
        if implementation != 0 {
            continue; // in another file or assembly: not embedded here
        }
        let mut e = strings + name_ix;
        let start = e;
        while r.u8(e)? != 0 {
            e += 1;
        }
        let name = String::from_utf8(r.slice(start, e - start)?.to_vec()).map_err(|e| e.to_string())?;
        let base = resources.ok_or("an embedded resource but no resources directory")? + offset;
        let len = r.u32(base)? as usize;
        r.slice(base + 4, len)?;
        out.push((name, base + 4..base + 4 + len));
    }
    Ok(out)
}

struct Reader<'a> {
    b: &'a [u8],
}
impl Reader<'_> {
    fn slice(&self, at: usize, n: usize) -> Result<&[u8], String> {
        self.b.get(at..at.checked_add(n).ok_or("overflow")?).ok_or_else(|| format!("the file ends before byte {}", at + n))
    }
    fn u8(&self, at: usize) -> Result<u8, String> {
        Ok(self.slice(at, 1)?[0])
    }
    fn u16(&self, at: usize) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.slice(at, 2)?.try_into().unwrap()))
    }
    fn u32(&self, at: usize) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.slice(at, 4)?.try_into().unwrap()))
    }
    fn u64(&self, at: usize) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.slice(at, 8)?.try_into().unwrap()))
    }
}

#[cfg(test)]
mod tests {
    /// Every resource that the decompile extracted from the player's Terraria.exe equals the bytes read here, and the
    /// names come in the manifest order the port uses (port/crates/terraria-gen-game/tests/data/resources.txt).
    /// Skipped (with a message) where the install or the decompile is not on this machine.
    #[test]
    fn resources_of_the_game_assembly_match_the_extracted_files() {
        let install = std::env::var_os("TERRARIA_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| r"C:\Users\user\Documents\Games\Terraria".into());
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let (Ok(exe), true) = (std::fs::read(install.join("Terraria.exe")), root.join("decompile/Terraria").is_dir()) else {
            println!("skipped: no Terraria.exe or no decompile on this machine");
            return;
        };
        let list = super::manifest_resources(&exe).expect("parse");
        let order: Vec<&str> = include_str!("../../../terraria-gen-game/tests/data/resources.txt").lines().filter(|l| !l.starts_with('#') && !l.is_empty()).collect();
        let names: Vec<&str> = list.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, order, "manifest order");
        let mut compared = 0;
        for (name, range) in &list {
            if let Ok(file) = std::fs::read(root.join("decompile/Terraria").join(name)) {
                assert!(file == exe[range.clone()], "{name}: the bytes differ from the extracted file");
                compared += 1;
            }
        }
        assert!(compared > 50, "only {compared} resources had an extracted file");
        println!("{} resources, {compared} compared with the extracted files", list.len());
    }
}
