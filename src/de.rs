//! CBOR deserialisation tooling

use error::Error;
use len::Len;
use result::Result;
use std::{self, collections::BTreeMap, io::BufRead};
use types::{Special, Type};

pub trait Deserialize: Sized {
    /// method to implement to deserialise an object from the given
    /// `Deserializer`.
    fn deserialize<R: BufRead>(reader: &mut Deserializer<R>) -> Result<Self>;
}

impl Deserialize for u8 {
    fn deserialize<R: BufRead>(raw: &mut Deserializer<R>) -> Result<Self> {
        let n = raw.unsigned_integer()?;
        if n > std::u8::MAX as u64 {
            Err(Error::ExpectedU8)
        } else {
            Ok(n as Self)
        }
    }
}

impl Deserialize for u16 {
    fn deserialize<R: BufRead>(raw: &mut Deserializer<R>) -> Result<Self> {
        let n = raw.unsigned_integer()?;
        if n > std::u16::MAX as u64 {
            Err(Error::ExpectedU16)
        } else {
            Ok(n as Self)
        }
    }
}

impl Deserialize for u32 {
    fn deserialize<R: BufRead>(raw: &mut Deserializer<R>) -> Result<Self> {
        let n = raw.unsigned_integer()?;
        if n > std::u32::MAX as u64 {
            Err(Error::ExpectedU32)
        } else {
            Ok(n as Self)
        }
    }
}

impl Deserialize for u64 {
    fn deserialize<R: BufRead>(raw: &mut Deserializer<R>) -> Result<Self> {
        raw.unsigned_integer()
    }
}

impl Deserialize for bool {
    fn deserialize<R: BufRead>(raw: &mut Deserializer<R>) -> Result<Self> {
        raw.bool()
    }
}

impl Deserialize for String {
    fn deserialize<R: BufRead>(raw: &mut Deserializer<R>) -> Result<Self> {
        raw.text()
    }
}

impl<T: Deserialize> Deserialize for Vec<T> {
    fn deserialize<R: BufRead>(raw: &mut Deserializer<R>) -> Result<Self> {
        let mut vec = Vec::new();
        raw.array_with(|raw| {
            vec.push(Deserialize::deserialize(raw)?);
            Ok(())
        })?;
        Ok(vec)
    }
}
impl<K: Deserialize + Ord, V: Deserialize> Deserialize for BTreeMap<K, V> {
    fn deserialize<R: BufRead>(raw: &mut Deserializer<R>) -> Result<Self> {
        let mut vec = BTreeMap::new();
        raw.map_with(|raw| {
            let k = Deserialize::deserialize(raw)?;
            let v = Deserialize::deserialize(raw)?;
            vec.insert(k, v);
            Ok(())
        })?;
        Ok(vec)
    }
}

impl<T: Deserialize> Deserialize for Option<T> {
    fn deserialize<R: BufRead>(raw: &mut Deserializer<R>) -> Result<Self> {
        match raw.array()? {
            Len::Len(0) => Ok(None),
            Len::Len(1) => Ok(Some(raw.deserialize()?)),
            len => Err(Error::CustomError(format!(
                "Invalid Option<T>: received array of {:?} elements",
                len
            ))),
        }
    }
}

/// [`Deserialize`]: ./trait.Deserialize.html
/// [`Error`]: ../enum.Error.html
/// [`Type`]: ../enum.Type.html
/// [`Bytes`]: ../struct.Bytes.html
/// [`cbor_type`]: #method.cbor_type
/// [`cbor_len`]: #method.cbor_len
/// [`Len`]: ../enum.Len.html
///
/// `Deserializer` represents a chunk of bytes believed to be cbor object.
/// The validity of the cbor bytes is known only when trying
/// to get meaningful cbor objects from it.
///
/// # Examples
///
/// If you already know the CBOR Primary [`Type`] you are expecting, you
/// can efficiently use the appropriate commands:
///
/// ```
/// use cbor_event::de::*;
/// use std::io::Cursor;
///
/// let vec = vec![0x18, 0x40];
/// let mut raw = Deserializer::from(Cursor::new(vec));
///
/// assert!(raw.unsigned_integer().is_ok());
/// ```
///
/// ```
/// use cbor_event::de::*;
/// use std::io::Cursor;
///
/// let vec = vec![0x18, 0x40];
/// let mut raw = Deserializer::from(Cursor::new(vec));
///
/// assert!(raw.array().is_err());
/// ```
///
/// If you don't know the [`Type`] and are only analyzing the structure, you
/// can use [`cbor_type`] to get the type of the next object to parse.
///
/// # Error
///
/// When deserialising from `Deserializer` it is possible to see the following
/// [`Error`]s:
///
/// - `Error::NotEnough(current_size, needed_size)`: meaning we are expecting
///   a more bytes to parse the CBOR properly;
/// - `Error::Expected(expected_type, current_type)`: the current cbor primary
///   [`Type`] is different from the expected [`Type`];
/// - `Error::UnknownLenType(byte)`: the CBOR is serialized in an unknown
///   or unsupported format;
/// - `Error::IndefiniteLenUnsupported(t)`: the Indefinite length is not
///   supported for the given [`Type`] `t`;
/// - `Error::IoError(io_error)`: error due relating to buffer management;
///
/// # Panic
///
/// There is no explicit `panic!` in this code, except a few `unreachable!`.
///
pub struct Deserializer<R>(R);
impl<R> From<R> for Deserializer<R> {
    fn from(r: R) -> Self {
        Deserializer(r)
    }
}
impl<R> Deserializer<R> {
    pub fn as_ref(&self) -> &R {
        &self.0
    }
    pub fn as_mut_ref(&mut self) -> &mut R {
        &mut self.0
    }
    pub fn inner(self) -> R {
        self.0
    }
}
impl<R: BufRead> Deserializer<R> {
    #[inline]
    fn get(&mut self, index: usize) -> Result<u8> {
        let buf = self.0.fill_buf()?;
        match buf.get(index) {
            None => Err(Error::NotEnough(buf.len(), index)),
            Some(b) => Ok(*b),
        }
    }
    #[inline]
    fn u8(&mut self, index: usize) -> Result<u64> {
        let b = self.get(index)?;
        Ok(b as u64)
    }
    #[inline]
    fn u16(&mut self, index: usize) -> Result<u64> {
        let b1 = self.u8(index)?;
        let b2 = self.u8(index + 1)?;
        Ok(b1 << 8 | b2)
    }
    #[inline]
    fn u32(&mut self, index: usize) -> Result<u64> {
        let b1 = self.u8(index)?;
        let b2 = self.u8(index + 1)?;
        let b3 = self.u8(index + 2)?;
        let b4 = self.u8(index + 3)?;
        Ok(b1 << 24 | b2 << 16 | b3 << 8 | b4)
    }
    #[inline]
    fn u64(&mut self, index: usize) -> Result<u64> {
        let b1 = self.u8(index)?;
        let b2 = self.u8(index + 1)?;
        let b3 = self.u8(index + 2)?;
        let b4 = self.u8(index + 3)?;
        let b5 = self.u8(index + 4)?;
        let b6 = self.u8(index + 5)?;
        let b7 = self.u8(index + 6)?;
        let b8 = self.u8(index + 7)?;
        Ok(b1 << 56 | b2 << 48 | b3 << 40 | b4 << 32 | b5 << 24 | b6 << 16 | b7 << 8 | b8)
    }

    /// function to extract the type of the given `Deserializer`.
    ///
    /// This function does not consume the underlying buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// use cbor_event::{de::*, Type};
    /// use std::io::Cursor;
    ///
    /// let vec = vec![0x18, 0x40];
    /// let mut raw = Deserializer::from(Cursor::new(vec));
    /// let cbor_type = raw.cbor_type().unwrap();
    ///
    /// assert!(cbor_type == Type::UnsignedInteger);
    /// ```
    #[inline]
    pub fn cbor_type(&mut self) -> Result<Type> {
        Ok(Type::from(self.get(0)?))
    }
    #[inline]
    fn cbor_expect_type(&mut self, t: Type) -> Result<()> {
        let t_ = self.cbor_type()?;
        if t_ != t {
            Err(Error::Expected(t, t_))
        } else {
            Ok(())
        }
    }

    /// function to extract the get the length parameter of
    /// the given cbor object. The returned tuple contains
    ///
    /// [`Type`]: ../enum.Type.html
    /// [`Len`]: ../enum.Len.html
    ///
    /// * the [`Len`];
    /// * the size of the encoded length (the number of bytes the data was encoded in).
    ///   `0` means the length is `< 24` and was encoded along the `Type`.
    ///
    /// If you are expecting a `Type` `UnsignedInteger` or `NegativeInteger` the meaning of
    /// the length is slightly different:
    ///
    /// * `Len::Indefinite` is an error;
    /// * `Len::Len(len)` is the read value of the integer.
    ///
    /// This function does not consume the underlying buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// use cbor_event::{de::*, Len};
    /// use std::io::Cursor;
    ///
    /// let vec = vec![0x83, 0x01, 0x02, 0x03];
    /// let mut raw = Deserializer::from(Cursor::new(vec));
    /// let (len, len_sz) = raw.cbor_len().unwrap();
    ///
    /// assert_eq!(len, Len::Len(3));
    /// assert_eq!(len_sz, 0);
    /// ```
    ///
    #[inline]
    pub fn cbor_len(&mut self) -> Result<(Len, usize)> {
        let b: u8 = self.get(0)? & 0b0001_1111;
        match b {
            0x00..=0x17 => Ok((Len::Len(b as u64), 0)),
            0x18 => self.u8(1).map(|v| (Len::Len(v), 1)),
            0x19 => self.u16(1).map(|v| (Len::Len(v), 2)),
            0x1a => self.u32(1).map(|v| (Len::Len(v), 4)),
            0x1b => self.u64(1).map(|v| (Len::Len(v), 8)),
            0x1c..=0x1e => Err(Error::UnknownLenType(b)),
            0x1f => Ok((Len::Indefinite, 0)),

            // since the value `b` has been masked to only consider the first 5 lowest bits
            // all value above 0x1f are unreachable.
            _ => unreachable!(),
        }
    }

    /// consume the given `len` from the underlying buffer. Skipped bytes are
    /// then lost, they cannot be retrieved for future references.
    #[inline]
    pub fn advance(&mut self, len: usize) -> Result<()> {
        Ok(self.0.consume(len))
    }

    /// Read an `UnsignedInteger` from the `Deserializer`
    ///
    /// The function fails if the type of the given Deserializer is not `Type::UnsignedInteger`.
    ///
    /// # Example
    ///
    /// ```
    /// use cbor_event::de::{*};
    /// use std::io::Cursor;
    ///
    /// let vec = vec![0x18, 0x40];
    /// let mut raw = Deserializer::from(Cursor::new(vec));
    ///
    /// let integer = raw.unsigned_integer().unwrap();
    ///
    /// assert_eq!(integer, 64);
    /// ```
    ///
    /// ```should_panic
    /// use cbor_event::de::{*};
    /// use std::io::Cursor;
    ///
    /// let vec = vec![0x83, 0x01, 0x02, 0x03];
    /// let mut raw = Deserializer::from(Cursor::new(vec));
    ///
    /// // the following line will panic:
    /// let integer = raw.unsigned_integer().unwrap();
    /// ```
    pub fn unsigned_integer(&mut self) -> Result<u64> {
        self.cbor_expect_type(Type::UnsignedInteger)?;
        let (len, len_sz) = self.cbor_len()?;
        match len {
            Len::Indefinite => Err(Error::IndefiniteLenNotSupported(Type::UnsignedInteger)),
            Len::Len(v) => {
                self.advance(1 + len_sz)?;
                Ok(v)
            }
        }
    }

    /// Read a `NegativeInteger` from the `Deserializer`
    ///
    /// The function fails if the type of the given Deserializer is not `Type::NegativeInteger`.
    ///
    /// # Example
    ///
    /// ```
    /// use cbor_event::de::{*};
    /// use std::io::Cursor;
    ///
    /// let vec = vec![0x38, 0x29];
    /// let mut raw = Deserializer::from(Cursor::new(vec));
    ///
    /// let integer = raw.negative_integer().unwrap();
    ///
    /// assert_eq!(integer, -42);
    /// ```
    pub fn negative_integer(&mut self) -> Result<i64> {
        self.cbor_expect_type(Type::NegativeInteger)?;
        let (len, len_sz) = self.cbor_len()?;
        match len {
            Len::Indefinite => Err(Error::IndefiniteLenNotSupported(Type::NegativeInteger)),
            Len::Len(v) => {
                self.advance(1 + len_sz)?;
                Ok(-(v as i64) - 1)
            }
        }
    }

    /// Read a Bytes from the Deserializer
    ///
    /// The function fails if the type of the given Deserializer is not `Type::Bytes`.
    ///
    /// # Example
    ///
    /// ```
    /// use cbor_event::de::{*};
    /// use std::io::Cursor;
    ///
    /// let vec = vec![0x52, 0x73, 0x6F, 0x6D, 0x65, 0x20, 0x72, 0x61, 0x6E, 0x64, 0x6F, 0x6D, 0x20, 0x73, 0x74, 0x72, 0x69, 0x6E, 0x67];
    /// let mut raw = Deserializer::from(Cursor::new(vec));
    ///
    /// let bytes = raw.bytes().unwrap();
    /// ```
    pub fn bytes<'a>(&'a mut self) -> Result<Vec<u8>> {
        use std::io::Read;

        self.cbor_expect_type(Type::Bytes)?;
        let (len, len_sz) = self.cbor_len()?;
        self.advance(1 + len_sz)?;
        match len {
            Len::Indefinite => {
                let mut bytes = vec![];
                while self.cbor_type()? != Type::Special || !self.special_break()? {
                    self.cbor_expect_type(Type::Bytes)?;
                    let (chunk_len, chunk_len_sz) = self.cbor_len()?;
                    match chunk_len {
                        Len::Indefinite => return Err(Error::InvalidIndefiniteString),
                        Len::Len(len) => {
                            self.advance(1 + chunk_len_sz)?;
                            self.0.by_ref().take(len).read_to_end(&mut bytes)?;
                        }
                    }
                }
                Ok(bytes)
            }
            Len::Len(len) => {
                let mut bytes = vec![0; len as usize];
                self.0.read_exact(&mut bytes)?;
                Ok(bytes)
            }
        }
    }

    /// Read a Text from the Deserializer
    ///
    /// The function fails if the type of the given Deserializer is not `Type::Text`.
    ///
    /// # Example
    ///
    /// ```
    /// use cbor_event::de::{*};
    /// use std::io::Cursor;
    ///
    /// let vec = vec![0x64, 0x74, 0x65, 0x78, 0x74];
    /// let mut raw = Deserializer::from(Cursor::new(vec));
    ///
    /// let text = raw.text().unwrap();
    ///
    /// assert!(&*text == "text");
    /// ```
    pub fn text(&mut self) -> Result<String> {
        self.cbor_expect_type(Type::Text)?;
        let (len, len_sz) = self.cbor_len()?;
        self.advance(1 + len_sz)?;
        match len {
            Len::Indefinite => {
                let mut text = String::new();
                while self.cbor_type()? != Type::Special || !self.special_break()? {
                    self.cbor_expect_type(Type::Text)?;
                    let (chunk_len, chunk_len_sz) = self.cbor_len()?;
                    match chunk_len {
                        Len::Indefinite => return Err(Error::InvalidIndefiniteString),
                        Len::Len(len) => {
                            // rfc7049 forbids splitting UTF-8 characters across chunks so we must
                            // read each chunk separately as a definite encoded UTF-8 string
                            self.advance(1 + chunk_len_sz)?;
                            let mut bytes = vec![0; len as usize];
                            self.0.read_exact(&mut bytes)?;
                            let chunk_text = String::from_utf8(bytes)?;
                            text.push_str(&chunk_text);
                        }
                    }
                }
                Ok(text)
            }
            Len::Len(len) => {
                let mut bytes = vec![0; len as usize];
                self.0.read_exact(&mut bytes)?;
                let text = String::from_utf8(bytes)?;
                Ok(text)
            }
        }
    }

    // Internal helper to decode a series of `len` items using a function. If
    // `len` is indefinite, decode until a `Special::Break`. If `len` is
    // definite, decode that many items.
    fn internal_items_with<F>(&mut self, len: Len, mut f: F) -> Result<()>
    where
        F: FnMut(&mut Self) -> Result<()>,
    {
        match len {
            Len::Indefinite => {
                while !self.special_break()? {
                    f(self)?;
                }
            }
            Len::Len(len) => {
                for _ in 0..len {
                    f(self)?;
                }
            }
        }
        Ok(())
    }

    /// cbor array of cbor objects
    ///
    /// The function fails if the type of the given Deserializer is not `Type::Array`.
    ///
    /// # Example
    ///
    /// ```
    /// use cbor_event::{de::{*}, Len};
    /// use std::io::Cursor;
    ///
    /// let vec = vec![0x86, 0,1,2,3,4,5];
    /// let mut raw = Deserializer::from(Cursor::new(vec));
    ///
    /// let len = raw.array().unwrap();
    ///
    /// assert_eq!(len, Len::Len(6));
    /// ```
    ///
    pub fn array(&mut self) -> Result<Len> {
        self.cbor_expect_type(Type::Array)?;
        let (len, sz) = self.cbor_len()?;
        self.advance(1 + sz)?;
        Ok(len)
    }

    /// Helper to decode a cbor array using a specified function.
    ///
    /// This works with either definite or indefinite arrays. Each call to the
    /// function should decode one item. If the function returns an error,
    /// decoding stops and returns that error.
    pub fn array_with<F>(&mut self, f: F) -> Result<()>
    where
        F: FnMut(&mut Self) -> Result<()>,
    {
        let len = self.array()?;
        self.internal_items_with(len, f)
    }

    /// Expect an array of a specified length. Must be a definite-length array.
    pub fn tuple(&mut self, expected_len: u64, error_location: &'static str) -> Result<()> {
        let actual_len = self.array()?;
        match actual_len {
            Len::Len(len) if expected_len == len => Ok(()),
            _ => Err(Error::WrongLen(expected_len, actual_len, error_location)),
        }
    }

    /// cbor map
    ///
    /// The function fails if the type of the given Deserializer is not `Type::Map`.
    ///
    /// # Example
    ///
    /// ```
    /// use cbor_event::{de::{*}, Len};
    /// use std::io::Cursor;
    ///
    /// let vec = vec![0xA2, 0x00, 0x64, 0x74, 0x65, 0x78, 0x74, 0x01, 0x18, 0x2A];
    /// let mut raw = Deserializer::from(Cursor::new(vec));
    ///
    /// let len = raw.map().unwrap();
    ///
    /// assert_eq!(len, Len::Len(2));
    /// ```
    ///
    pub fn map(&mut self) -> Result<Len> {
        self.cbor_expect_type(Type::Map)?;
        let (len, sz) = self.cbor_len()?;
        self.advance(1 + sz)?;
        Ok(len)
    }

    /// Helper to decode a cbor map using a specified function
    ///
    /// This works with either definite or indefinite maps. Each call to the
    /// function should decode one key followed by one value. If the function
    /// returns an error, decoding stops and returns that error.
    pub fn map_with<F>(&mut self, f: F) -> Result<()>
    where
        F: FnMut(&mut Self) -> Result<()>,
    {
        let len = self.map()?;
        self.internal_items_with(len, f)
    }

    /// Cbor Tag
    ///
    /// The function fails if the type of the given Deserializer is not `Type::Tag`.
    ///
    /// # Example
    ///
    /// ```
    /// use std::io::Cursor;
    /// use cbor_event::{de::{*}, Len};
    ///
    /// let vec = vec![0xD8, 0x18, 0x64, 0x74, 0x65, 0x78, 0x74];
    /// let mut raw = Deserializer::from(Cursor::new(vec));
    ///
    /// let tag = raw.tag().unwrap();
    ///
    /// assert_eq!(24, tag);
    /// assert_eq!("text", &*raw.text().unwrap());
    /// ```
    ///
    pub fn tag(&mut self) -> Result<u64> {
        self.cbor_expect_type(Type::Tag)?;
        match self.cbor_len()? {
            (Len::Indefinite, _) => Err(Error::IndefiniteLenNotSupported(Type::Tag)),
            (Len::Len(len), sz) => {
                self.advance(1 + sz)?;
                Ok(len)
            }
        }
    }

    pub fn set_tag(&mut self) -> Result<()> {
        let tag = self.tag()?;
        if tag != 258 {
            return Err(Error::ExpectedSetTag);
        }
        Ok(())
    }

    /// If the next byte is a `Special::Break`, advance past it and return `true`; otherwise,
    /// return `false` without advancing.
    ///
    /// Useful when decoding a variable-length array or map where the items may themselves use
    /// `Special`, such as bool values.
    pub fn special_break(&mut self) -> Result<bool> {
        self.cbor_expect_type(Type::Special)?;
        let b = self.get(0)? & 0b0001_1111;
        if b == 0x1f {
            self.advance(1)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn special(&mut self) -> Result<Special> {
        self.cbor_expect_type(Type::Special)?;
        let b = self.get(0)? & 0b0001_1111;
        match b {
            0x00..=0x13 => {
                self.advance(1)?;
                Ok(Special::Unassigned(b))
            }
            0x14 => {
                self.advance(1)?;
                Ok(Special::Bool(false))
            }
            0x15 => {
                self.advance(1)?;
                Ok(Special::Bool(true))
            }
            0x16 => {
                self.advance(1)?;
                Ok(Special::Null)
            }
            0x17 => {
                self.advance(1)?;
                Ok(Special::Undefined)
            }
            0x18 => {
                let b = self.u8(1)?;
                self.advance(2)?;
                Ok(Special::Unassigned(b as u8))
            }
            0x19 => {
                let f = self.u16(1)?;
                self.advance(3)?;
                Ok(Special::Float(f as f64))
            }
            0x1a => {
                let f = self.u32(1)?;
                self.advance(5)?;
                Ok(Special::Float(f as f64))
            }
            0x1b => {
                let f = self.u64(1)?;
                self.advance(9)?;
                Ok(Special::Float(f as f64))
            }
            0x1c..=0x1e => {
                self.advance(1)?;
                Ok(Special::Unassigned(b))
            }
            0x1f => {
                self.advance(1)?;
                Ok(Special::Break)
            }
            _ => unreachable!(),
        }
    }

    pub fn bool(&mut self) -> Result<bool> {
        self.special()?.unwrap_bool()
    }

    pub fn deserialize<T>(&mut self) -> Result<T>
    where
        T: Deserialize,
    {
        Deserialize::deserialize(self)
    }

    /// Deserialize a value of type `T` and check that there is no
    /// trailing data.
    pub fn deserialize_complete<T>(&mut self) -> Result<T>
    where
        T: Deserialize,
    {
        let v = self.deserialize()?;
        if self.0.fill_buf()?.len() > 0 {
            Err(Error::TrailingData)
        } else {
            Ok(v)
        }
    }
}

// deserialisation macro

macro_rules! deserialize_array {
    ( $( $x:expr ),* ) => {
        $(
            impl Deserialize for [u8; $x] {
                fn deserialize<R: BufRead>(raw: &mut Deserializer<R>) -> Result<Self> {
                    let mut bytes = [0u8; $x];

                    let len = raw.array()?;
                    match len {
                        Len::Indefinite => {
                            return Err(Error::WrongLen($x, len, "static array"));
                        },
                        Len::Len(x) => {
                            if x != $x {
                                return Err(Error::WrongLen($x, len, "static array"));
                            }
                        }
                    }

                    for byte in bytes.iter_mut() {
                        *byte = Deserialize::deserialize(raw)?;
                    }
                    Ok(bytes)
                }
            }
        )*
    }
}

deserialize_array!(
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
    27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50,
    51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64
);

#[cfg(test)]
mod test {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn negative_integer() {
        let vec = vec![0x38, 0x29];
        let mut raw = Deserializer::from(Cursor::new(vec));

        let integer = raw.negative_integer().unwrap();

        assert_eq!(integer, -42);
    }

    #[test]
    fn bytes() {
        let vec = vec![
            0x52, 0x73, 0x6F, 0x6D, 0x65, 0x20, 0x72, 0x61, 0x6E, 0x64, 0x6F, 0x6D, 0x20, 0x73,
            0x74, 0x72, 0x69, 0x6E, 0x67,
        ];
        let mut raw = Deserializer::from(Cursor::new(vec.clone()));

        let bytes = raw.bytes().unwrap();
        assert_eq!(&vec[1..], &*bytes);
    }
    #[test]
    fn bytes_indefinite() {
        let chunks = vec![
            vec![
                0x52, 0x73, 0x6F, 0x6D, 0x65, 0x20, 0x72, 0x61, 0x6E, 0x64, 0x6F, 0x6D, 0x20, 0x73,
                0x74, 0x72, 0x69, 0x6E, 0x67,
            ],
            vec![0x44, 0x01, 0x02, 0x03, 0x04],
        ];
        let mut expected = Vec::new();
        for chunk in chunks.iter() {
            expected.extend_from_slice(&chunk[1..]);
        }
        let mut vec = vec![0x5f];
        for mut chunk in chunks {
            vec.append(&mut chunk);
        }
        vec.push(0xff);
        let mut raw = Deserializer::from(Cursor::new(vec.clone()));
        let found = raw.bytes().unwrap();
        assert_eq!(found, expected);
    }
    #[test]
    fn bytes_empty() {
        let vec = vec![0x40];
        let mut raw = Deserializer::from(Cursor::new(vec));

        let bytes = raw.bytes().unwrap();
        assert!(bytes.is_empty());
    }

    #[test]
    fn text() {
        let vec = vec![0x64, 0x74, 0x65, 0x78, 0x74];
        let mut raw = Deserializer::from(Cursor::new(vec));

        let text = raw.text().unwrap();

        assert_eq!(&text, "text");
    }
    #[test]
    fn text_indefinite() {
        let chunks = vec![vec![0x64, 0x49, 0x45, 0x54, 0x46], vec![0x61, 0x61]];
        let expected = "IETFa";
        let mut vec = vec![0x7f];
        for mut chunk in chunks {
            vec.append(&mut chunk);
        }
        vec.push(0xff);
        let mut raw = Deserializer::from(Cursor::new(vec.clone()));
        let found = raw.text().unwrap();
        assert_eq!(found, expected);
    }
    #[test]
    fn text_empty() {
        let vec = vec![0x60];
        let mut raw = Deserializer::from(Cursor::new(vec));

        let text = raw.text().unwrap();

        assert_eq!(&text, "");
    }

    #[test]
    fn array() {
        let vec = vec![0x86, 0, 1, 2, 3, 4, 5];
        let mut raw = Deserializer::from(Cursor::new(vec));

        let len = raw.array().unwrap();

        assert_eq!(len, Len::Len(6));
        // assert_eq!(&*raw, &[0, 1, 2, 3, 4, 5][..]);

        assert_eq!(0, raw.unsigned_integer().unwrap());
        assert_eq!(1, raw.unsigned_integer().unwrap());
        assert_eq!(2, raw.unsigned_integer().unwrap());
        assert_eq!(3, raw.unsigned_integer().unwrap());
        assert_eq!(4, raw.unsigned_integer().unwrap());
        assert_eq!(5, raw.unsigned_integer().unwrap());
    }
    #[test]
    fn array_empty() {
        let vec = vec![0x80];
        let mut raw = Deserializer::from(Cursor::new(vec));

        let len = raw.array().unwrap();

        assert_eq!(len, Len::Len(0));
        // assert_eq!(&*raw, &[][..]);
    }
    #[test]
    fn array_indefinite() {
        let vec = vec![0x9F, 0x01, 0x02, 0xFF];
        let mut raw = Deserializer::from(Cursor::new(vec));

        let len = raw.array().unwrap();

        assert_eq!(len, Len::Indefinite);
        // assert_eq!(&*raw, &[0x01, 0x02, 0xFF][..]);

        let i = raw.unsigned_integer().unwrap();
        assert!(i == 1);
        let i = raw.unsigned_integer().unwrap();
        assert!(i == 2);
        assert_eq!(Special::Break, raw.special().unwrap());
    }

    #[test]
    fn vec_bool_definite() {
        let vec = vec![0x83, 0xf4, 0xf5, 0xf4];
        let mut raw = Deserializer::from(Cursor::new(vec));
        let bools = Vec::<bool>::deserialize(&mut raw).unwrap();
        assert_eq!(bools, &[false, true, false]);
    }
    #[test]
    fn vec_bool_indefinite() {
        let vec = vec![0x9f, 0xf4, 0xf5, 0xf4, 0xff];
        let mut raw = Deserializer::from(Cursor::new(vec));
        let bools = Vec::<bool>::deserialize(&mut raw).unwrap();
        assert_eq!(bools, &[false, true, false]);
    }

    #[test]
    fn complex_array() {
        let vec = vec![
            0x85, 0x64, 0x69, 0x6F, 0x68, 0x6B, 0x01, 0x20, 0x84, 0, 1, 2, 3, 0x10,
            /* garbage... */ 0, 1, 2, 3, 4, 5, 6,
        ];
        let mut raw = Deserializer::from(Cursor::new(vec));

        let len = raw.array().unwrap();

        assert_eq!(len, Len::Len(5));

        assert_eq!("iohk", &raw.text().unwrap());
        assert_eq!(1, raw.unsigned_integer().unwrap());
        assert_eq!(-1, raw.negative_integer().unwrap());

        let nested_array_len = raw.array().unwrap();
        assert_eq!(nested_array_len, Len::Len(4));
        assert_eq!(0, raw.unsigned_integer().unwrap());
        assert_eq!(1, raw.unsigned_integer().unwrap());
        assert_eq!(2, raw.unsigned_integer().unwrap());
        assert_eq!(3, raw.unsigned_integer().unwrap());

        assert_eq!(0x10, raw.unsigned_integer().unwrap());

        const GARBAGE_LEN: usize = 7;
        // assert_eq!(GARBAGE_LEN, raw.len());
    }

    #[test]
    fn map() {
        let vec = vec![0xA2, 0x00, 0x64, 0x74, 0x65, 0x78, 0x74, 0x01, 0x18, 0x2A];
        let mut raw = Deserializer::from(Cursor::new(vec));

        let len = raw.map().unwrap();

        assert_eq!(len, Len::Len(2));

        let k = raw.unsigned_integer().unwrap();
        let v = raw.text().unwrap();
        assert_eq!(0, k);
        assert_eq!("text", &v);

        let k = raw.unsigned_integer().unwrap();
        let v = raw.unsigned_integer().unwrap();
        assert_eq!(1, k);
        assert_eq!(42, v);
    }

    #[test]
    fn map_empty() {
        let vec = vec![0xA0];
        let mut raw = Deserializer::from(Cursor::new(vec));

        let len = raw.map().unwrap();

        assert_eq!(len, Len::Len(0));
    }

    #[test]
    fn btreemap_bool_definite() {
        let vec = vec![0xa2, 0xf4, 0xf5, 0xf5, 0xf4];
        let mut raw = Deserializer::from(Cursor::new(vec));
        let boolmap = BTreeMap::<bool, bool>::deserialize(&mut raw).unwrap();
        assert_eq!(boolmap.len(), 2);
        assert_eq!(boolmap[&false], true);
        assert_eq!(boolmap[&true], false);
    }
    #[test]
    fn btreemap_bool_indefinite() {
        let vec = vec![0xbf, 0xf4, 0xf5, 0xf5, 0xf4, 0xff];
        let mut raw = Deserializer::from(Cursor::new(vec));
        let boolmap = BTreeMap::<bool, bool>::deserialize(&mut raw).unwrap();
        assert_eq!(boolmap.len(), 2);
        assert_eq!(boolmap[&false], true);
        assert_eq!(boolmap[&true], false);
    }

    #[test]
    fn tag() {
        let vec = vec![
            0xD8, 0x18, 0x52, 0x73, 0x6F, 0x6D, 0x65, 0x20, 0x72, 0x61, 0x6E, 0x64, 0x6F, 0x6D,
            0x20, 0x73, 0x74, 0x72, 0x69, 0x6E, 0x67,
        ];
        let mut raw = Deserializer::from(Cursor::new(vec));

        let tag = raw.tag().unwrap();

        assert_eq!(24, tag);
        let tagged = raw.bytes().unwrap();
        assert_eq!(b"some random string", &*tagged);
    }

    #[test]
    fn tag2() {
        let vec = vec![
            0x82, 0xd8, 0x18, 0x53, 0x52, 0x73, 0x6f, 0x6d, 0x65, 0x20, 0x72, 0x61, 0x6e, 0x64,
            0x6f, 0x6d, 0x20, 0x73, 0x74, 0x72, 0x69, 0x6e, 0x67, 0x1a, 0x71, 0xad, 0x58, 0x36,
        ];
        let mut raw = Deserializer::from(Cursor::new(vec));

        let len = raw.array().unwrap();
        assert_eq!(len, Len::Len(2));

        let tag = raw.tag().unwrap();
        assert!(tag == 24);
        let _ = raw.bytes().unwrap();

        let crc = raw.unsigned_integer().unwrap();
        assert!(crc as u32 == 0x71AD5836);
    }
}
