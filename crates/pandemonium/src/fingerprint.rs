//! Fingerprints of serializable values: a number that differs between two values that
//! would be written differently, made by feeding what serde walks of a value straight to a
//! hasher, without formatting it as text.

use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};

use serde::Serialize;
use serde::ser;

/// The fingerprint of `value`: two values that serialize the same have the same one.
pub fn of(value: &impl Serialize) -> u64 {
    let mut hasher = DefaultHasher::new();
    let _ = value.serialize(Feed(&mut hasher));
    hasher.finish()
}

/// Why feeding a value to a hasher stopped, which only a value's own serializer can say.
#[derive(Debug)]
pub struct Stopped(String);

impl fmt::Display for Stopped {
    /// Writes why feeding stopped.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Stopped {}

impl ser::Error for Stopped {
    /// The value's serializer stopped for `message`.
    fn custom<T: fmt::Display>(message: T) -> Self {
        Self(message.to_string())
    }
}

/// A serde serializer that hands every part of a value, with a tag for its kind, to a
/// hasher.
struct Feed<'a>(&'a mut DefaultHasher);

impl Feed<'_> {
    /// Hands `part`, tagged `kind`, to the hasher and goes on.
    fn take(self, kind: u8, part: impl Hash) -> Result<(), Stopped> {
        kind.hash(self.0);
        part.hash(self.0);
        Ok(())
    }

    /// A feed of the same hasher for a part inside a compound value.
    fn inner(&mut self) -> Feed<'_> {
        Feed(self.0)
    }
}

impl<'a> ser::Serializer for Feed<'a> {
    type Ok = ();
    type Error = Stopped;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    /// Feeds a flag.
    fn serialize_bool(self, value: bool) -> Result<(), Stopped> {
        self.take(1, value)
    }

    /// Feeds a small signed number.
    fn serialize_i8(self, value: i8) -> Result<(), Stopped> {
        self.take(2, i64::from(value))
    }

    /// Feeds a signed number.
    fn serialize_i16(self, value: i16) -> Result<(), Stopped> {
        self.take(2, i64::from(value))
    }

    /// Feeds a signed number.
    fn serialize_i32(self, value: i32) -> Result<(), Stopped> {
        self.take(2, i64::from(value))
    }

    /// Feeds a signed number.
    fn serialize_i64(self, value: i64) -> Result<(), Stopped> {
        self.take(2, value)
    }

    /// Feeds a small unsigned number.
    fn serialize_u8(self, value: u8) -> Result<(), Stopped> {
        self.take(3, u64::from(value))
    }

    /// Feeds an unsigned number.
    fn serialize_u16(self, value: u16) -> Result<(), Stopped> {
        self.take(3, u64::from(value))
    }

    /// Feeds an unsigned number.
    fn serialize_u32(self, value: u32) -> Result<(), Stopped> {
        self.take(3, u64::from(value))
    }

    /// Feeds an unsigned number.
    fn serialize_u64(self, value: u64) -> Result<(), Stopped> {
        self.take(3, value)
    }

    /// Feeds a fraction by its bits.
    fn serialize_f32(self, value: f32) -> Result<(), Stopped> {
        self.take(4, f64::from(value).to_bits())
    }

    /// Feeds a fraction by its bits.
    fn serialize_f64(self, value: f64) -> Result<(), Stopped> {
        self.take(4, value.to_bits())
    }

    /// Feeds a character.
    fn serialize_char(self, value: char) -> Result<(), Stopped> {
        self.take(5, value)
    }

    /// Feeds text.
    fn serialize_str(self, value: &str) -> Result<(), Stopped> {
        self.take(6, value)
    }

    /// Feeds bytes.
    fn serialize_bytes(self, value: &[u8]) -> Result<(), Stopped> {
        self.take(7, value)
    }

    /// Feeds the absence of a value.
    fn serialize_none(self) -> Result<(), Stopped> {
        self.take(8, ())
    }

    /// Feeds a value that is there.
    fn serialize_some<T: ?Sized + Serialize>(mut self, value: &T) -> Result<(), Stopped> {
        9u8.hash(self.0);
        value.serialize(self.inner())
    }

    /// Feeds the unit.
    fn serialize_unit(self) -> Result<(), Stopped> {
        self.take(10, ())
    }

    /// Feeds a unit struct by its name.
    fn serialize_unit_struct(self, name: &'static str) -> Result<(), Stopped> {
        self.take(11, name)
    }

    /// Feeds a unit variant by its position.
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        index: u32,
        _variant: &'static str,
    ) -> Result<(), Stopped> {
        self.take(12, index)
    }

    /// Feeds what a newtype struct wraps.
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<(), Stopped> {
        value.serialize(self)
    }

    /// Feeds a newtype variant by its position and what it wraps.
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        mut self,
        _name: &'static str,
        index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<(), Stopped> {
        (13u8, index).hash(self.0);
        value.serialize(self.inner())
    }

    /// Starts feeding a sequence.
    fn serialize_seq(self, length: Option<usize>) -> Result<Self, Stopped> {
        (14u8, length).hash(self.0);
        Ok(self)
    }

    /// Starts feeding a tuple.
    fn serialize_tuple(self, length: usize) -> Result<Self, Stopped> {
        (15u8, length).hash(self.0);
        Ok(self)
    }

    /// Starts feeding a tuple struct.
    fn serialize_tuple_struct(self, name: &'static str, length: usize) -> Result<Self, Stopped> {
        (16u8, name, length).hash(self.0);
        Ok(self)
    }

    /// Starts feeding a tuple variant by its position.
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        index: u32,
        _variant: &'static str,
        length: usize,
    ) -> Result<Self, Stopped> {
        (17u8, index, length).hash(self.0);
        Ok(self)
    }

    /// Starts feeding a map.
    fn serialize_map(self, length: Option<usize>) -> Result<Self, Stopped> {
        (18u8, length).hash(self.0);
        Ok(self)
    }

    /// Starts feeding a struct.
    fn serialize_struct(self, name: &'static str, length: usize) -> Result<Self, Stopped> {
        (19u8, name, length).hash(self.0);
        Ok(self)
    }

    /// Starts feeding a struct variant by its position.
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        index: u32,
        _variant: &'static str,
        length: usize,
    ) -> Result<Self, Stopped> {
        (20u8, index, length).hash(self.0);
        Ok(self)
    }
}

impl ser::SerializeSeq for Feed<'_> {
    type Ok = ();
    type Error = Stopped;

    /// Feeds an element.
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Stopped> {
        value.serialize(self.inner())
    }

    /// Ends the sequence.
    fn end(self) -> Result<(), Stopped> {
        Ok(())
    }
}

impl ser::SerializeTuple for Feed<'_> {
    type Ok = ();
    type Error = Stopped;

    /// Feeds an element.
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Stopped> {
        value.serialize(self.inner())
    }

    /// Ends the tuple.
    fn end(self) -> Result<(), Stopped> {
        Ok(())
    }
}

impl ser::SerializeTupleStruct for Feed<'_> {
    type Ok = ();
    type Error = Stopped;

    /// Feeds a field.
    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Stopped> {
        value.serialize(self.inner())
    }

    /// Ends the tuple struct.
    fn end(self) -> Result<(), Stopped> {
        Ok(())
    }
}

impl ser::SerializeTupleVariant for Feed<'_> {
    type Ok = ();
    type Error = Stopped;

    /// Feeds a field.
    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Stopped> {
        value.serialize(self.inner())
    }

    /// Ends the tuple variant.
    fn end(self) -> Result<(), Stopped> {
        Ok(())
    }
}

impl ser::SerializeMap for Feed<'_> {
    type Ok = ();
    type Error = Stopped;

    /// Feeds a key.
    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), Stopped> {
        key.serialize(self.inner())
    }

    /// Feeds a value.
    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Stopped> {
        value.serialize(self.inner())
    }

    /// Ends the map.
    fn end(self) -> Result<(), Stopped> {
        Ok(())
    }
}

impl ser::SerializeStruct for Feed<'_> {
    type Ok = ();
    type Error = Stopped;

    /// Feeds a field by its name and value.
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Stopped> {
        key.hash(self.0);
        value.serialize(self.inner())
    }

    /// Ends the struct.
    fn end(self) -> Result<(), Stopped> {
        Ok(())
    }
}

impl ser::SerializeStructVariant for Feed<'_> {
    type Ok = ();
    type Error = Stopped;

    /// Feeds a field by its name and value.
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Stopped> {
        key.hash(self.0);
        value.serialize(self.inner())
    }

    /// Ends the struct variant.
    fn end(self) -> Result<(), Stopped> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A value serialized for the test, with text, a fraction, a list and an absent part.
    #[derive(Serialize)]
    struct Sample {
        /// Some text.
        name: String,
        /// A fraction.
        share: f32,
        /// A list of numbers.
        rows: Vec<u32>,
        /// A part that may be absent.
        note: Option<String>,
    }

    /// The sample with `share` as its fraction and `note` as its note.
    fn sample(share: f32, note: Option<&str>) -> Sample {
        Sample {
            name: "view".to_owned(),
            share,
            rows: vec![1, 2, 3],
            note: note.map(str::to_owned),
        }
    }

    /// Equal values have equal fingerprints and a change of any part changes it.
    #[test]
    fn a_change_of_any_part_changes_the_fingerprint() {
        let base = of(&sample(0.5, None));
        assert_eq!(base, of(&sample(0.5, None)));
        assert_ne!(base, of(&sample(0.25, None)));
        assert_ne!(base, of(&sample(0.5, Some(""))));
    }
}
