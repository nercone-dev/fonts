use std::ffi::c_void;
use std::os::raw::{c_char, c_int, c_uint};

use write_fonts::types::Tag;

pub enum HbBlob {}
pub enum HbFace {}
pub enum HbSet {}
pub enum HbSubsetInput {}

pub const MEMORY_MODE_READONLY: c_int = 0;

pub const SETS_NO_SUBSET_TABLE_TAG: c_int = 2;
pub const SETS_DROP_TABLE_TAG: c_int = 3;
pub const SETS_LAYOUT_FEATURE_TAG: c_int = 6;
pub const SETS_LAYOUT_SCRIPT_TAG: c_int = 7;

pub const FLAGS_NO_HINTING: c_uint = 0x01;
pub const FLAGS_PASSTHROUGH_UNRECOGNIZED: c_uint = 0x20;
pub const FLAGS_NOTDEF_OUTLINE: c_uint = 0x40;
pub const FLAGS_GLYPH_NAMES: c_uint = 0x80;
pub const FLAGS_NO_PRUNE_UNICODE_RANGES: c_uint = 0x100;

extern "C" {
    pub fn hb_blob_create(data: *const c_char, length: c_uint, mode: c_int, user_data: *mut c_void, destroy: *mut c_void) -> *mut HbBlob;
    pub fn hb_blob_destroy(blob: *mut HbBlob);
    pub fn hb_blob_get_data(blob: *mut HbBlob, length: *mut c_uint) -> *const c_char;

    pub fn hb_face_create(blob: *mut HbBlob, index: c_uint) -> *mut HbFace;
    pub fn hb_face_destroy(face: *mut HbFace);
    pub fn hb_face_reference_blob(face: *mut HbFace) -> *mut HbBlob;

    pub fn hb_set_add(set: *mut HbSet, codepoint: c_uint);
    pub fn hb_set_clear(set: *mut HbSet);
    pub fn hb_set_invert(set: *mut HbSet);

    pub fn hb_subset_input_create_or_fail() -> *mut HbSubsetInput;
    pub fn hb_subset_input_destroy(input: *mut HbSubsetInput);
    pub fn hb_subset_input_unicode_set(input: *mut HbSubsetInput) -> *mut HbSet;
    pub fn hb_subset_input_glyph_set(input: *mut HbSubsetInput) -> *mut HbSet;
    pub fn hb_subset_input_set(input: *mut HbSubsetInput, set_type: c_int) -> *mut HbSet;
    pub fn hb_subset_input_set_flags(input: *mut HbSubsetInput, flags: c_uint);
    pub fn hb_subset_input_pin_axis_location(input: *mut HbSubsetInput, face: *mut HbFace, tag: c_uint, value: f32) -> c_int;
    pub fn hb_subset_input_set_axis_range(input: *mut HbSubsetInput, face: *mut HbFace, tag: c_uint, minimum: f32, maximum: f32, default: f32) -> c_int;
    pub fn hb_subset_or_fail(face: *mut HbFace, input: *const HbSubsetInput) -> *mut HbFace;
}

pub fn tag(value: Tag) -> c_uint {
    u32::from_be_bytes(value.to_be_bytes())
}

pub struct Face {
    pub raw: *mut HbFace,
}

impl Face {
    pub fn new(data: &[u8]) -> Face {
        unsafe {
            let blob = hb_blob_create(data.as_ptr() as *const c_char, data.len() as c_uint, MEMORY_MODE_READONLY, std::ptr::null_mut(), std::ptr::null_mut());
            let raw = hb_face_create(blob, 0);
            hb_blob_destroy(blob);
            Face { raw }
        }
    }

    pub fn data(&self) -> Vec<u8> {
        unsafe {
            let blob = hb_face_reference_blob(self.raw);
            let mut length: c_uint = 0;
            let bytes = hb_blob_get_data(blob, &mut length);
            let data = std::slice::from_raw_parts(bytes as *const u8, length as usize).to_vec();
            hb_blob_destroy(blob);
            data
        }
    }
}

impl Drop for Face {
    fn drop(&mut self) {
        unsafe { hb_face_destroy(self.raw) }
    }
}

pub struct SubsetInput {
    pub raw: *mut HbSubsetInput,
}

impl SubsetInput {
    pub fn new() -> SubsetInput {
        SubsetInput { raw: unsafe { hb_subset_input_create_or_fail() } }
    }

    pub fn fill(&self, set: *mut HbSet, values: impl IntoIterator<Item = u32>) {
        unsafe {
            hb_set_clear(set);
            for value in values {
                hb_set_add(set, value);
            }
        }
    }

    pub fn everything(&self, set: *mut HbSet) {
        unsafe {
            hb_set_clear(set);
            hb_set_invert(set);
        }
    }

    pub fn unicodes(&self, codepoints: impl IntoIterator<Item = u32>) -> &SubsetInput {
        self.fill(unsafe { hb_subset_input_unicode_set(self.raw) }, codepoints);
        self
    }

    pub fn all_glyphs(&self) -> &SubsetInput {
        self.everything(unsafe { hb_subset_input_glyph_set(self.raw) });
        self.everything(unsafe { hb_subset_input_unicode_set(self.raw) });
        self
    }

    pub fn layout_features(&self, tags: &[Tag]) -> &SubsetInput {
        self.fill(unsafe { hb_subset_input_set(self.raw, SETS_LAYOUT_FEATURE_TAG) }, tags.iter().map(|value| tag(*value)));
        self
    }

    pub fn all_layout_features(&self) -> &SubsetInput {
        self.everything(unsafe { hb_subset_input_set(self.raw, SETS_LAYOUT_FEATURE_TAG) });
        self
    }

    pub fn all_layout_scripts(&self) -> &SubsetInput {
        self.everything(unsafe { hb_subset_input_set(self.raw, SETS_LAYOUT_SCRIPT_TAG) });
        self
    }

    pub fn drop_tables(&self, tags: &[Tag]) -> &SubsetInput {
        unsafe {
            let set = hb_subset_input_set(self.raw, SETS_DROP_TABLE_TAG);
            for value in tags {
                hb_set_add(set, tag(*value));
            }
        }
        self
    }

    pub fn flags(&self, flags: c_uint) -> &SubsetInput {
        unsafe { hb_subset_input_set_flags(self.raw, flags) }
        self
    }

    pub fn pin_axis(&self, face: &Face, axis: Tag, value: f32) -> bool {
        unsafe { hb_subset_input_pin_axis_location(self.raw, face.raw, tag(axis), value) != 0 }
    }

    pub fn axis_range(&self, face: &Face, axis: Tag, minimum: f32, maximum: f32, default: f32) -> bool {
        unsafe { hb_subset_input_set_axis_range(self.raw, face.raw, tag(axis), minimum, maximum, default) != 0 }
    }

    pub fn subset(&self, face: &Face) -> Result<Vec<u8>, String> {
        unsafe {
            let result = hb_subset_or_fail(face.raw, self.raw);
            if result.is_null() {
                return Err("hb_subset_or_fail returned null".to_string());
            }
            let output = Face { raw: result };
            Ok(output.data())
        }
    }
}

impl Drop for SubsetInput {
    fn drop(&mut self) {
        unsafe { hb_subset_input_destroy(self.raw) }
    }
}

impl Default for SubsetInput {
    fn default() -> SubsetInput {
        SubsetInput::new()
    }
}
