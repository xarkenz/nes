use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

pub trait FromStrRadix {
    fn from_str_radix(src: &str, radix: u32) -> Result<Self, std::num::ParseIntError>
    where Self: Sized;
}

impl FromStrRadix for u8 {
    fn from_str_radix(src: &str, radix: u32) -> Result<Self, std::num::ParseIntError> {
        u8::from_str_radix(src, radix)
    }
}

impl FromStrRadix for u16 {
    fn from_str_radix(src: &str, radix: u32) -> Result<Self, std::num::ParseIntError> {
        u16::from_str_radix(src, radix)
    }
}

impl FromStrRadix for u32 {
    fn from_str_radix(src: &str, radix: u32) -> Result<Self, std::num::ParseIntError> {
        u32::from_str_radix(src, radix)
    }
}

pub fn parse_int<T: FromStrRadix>(string: &str) -> Result<T, std::num::ParseIntError> {
    let string = string.trim_start_matches('#');
    if let Some(hex) = string.strip_prefix("0x") {
        T::from_str_radix(hex, 16)
    }
    else if let Some(hex) = string.strip_prefix("$") {
        T::from_str_radix(hex, 16)
    }
    else if let Some(bin) = string.strip_prefix("0b") {
        T::from_str_radix(bin, 2)
    }
    else if let Some(bin) = string.strip_prefix("%") {
        T::from_str_radix(bin, 2)
    }
    else {
        T::from_str_radix(string, 10)
    }
}

pub struct AtomicF32(AtomicU32);

impl AtomicF32 {
    pub const fn new(value: f32) -> Self {
        Self(AtomicU32::new(unsafe { std::mem::transmute(value) }))
    }

    pub fn store(&self, value: f32, ordering: Ordering) {
        self.0.store(unsafe { std::mem::transmute(value) }, ordering);
    }

    pub fn swap(&self, value: f32, ordering: Ordering) -> f32 {
        let result = self.0.swap(unsafe { std::mem::transmute(value) }, ordering);
        unsafe { std::mem::transmute(result) }
    }

    pub fn load(&self, ordering: Ordering) -> f32 {
        unsafe { std::mem::transmute(self.0.load(ordering)) }
    }
}

pub struct AtomicF64(AtomicU64);

impl AtomicF64 {
    pub const fn new(value: f64) -> Self {
        Self(AtomicU64::new(unsafe { std::mem::transmute(value) }))
    }

    pub fn store(&self, value: f64, ordering: Ordering) {
        self.0.store(unsafe { std::mem::transmute(value) }, ordering);
    }

    pub fn swap(&self, value: f64, ordering: Ordering) -> f64 {
        let result = self.0.swap(unsafe { std::mem::transmute(value) }, ordering);
        unsafe { std::mem::transmute(result) }
    }

    pub fn load(&self, ordering: Ordering) -> f64 {
        unsafe { std::mem::transmute(self.0.load(ordering)) }
    }
}
