// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

//! Range
//!
//! <https://arrow.apache.org/docs/format/CanonicalExtensions.html#range>

use serde_core::de::{MapAccess, Visitor};
use serde_core::ser::SerializeStruct;
use serde_core::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{ArrowError, DataType, Fields, extension::ExtensionType};

/// Whether the endpoints of a range are included (closed) or excluded (open).
///
/// Uses the same vocabulary as pandas: "left", "right", "both", "neither".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeClosed {
    /// The left (lower) endpoint is included; the right (upper) is excluded.
    Left,
    /// The left (lower) endpoint is excluded; the right (upper) is included.
    Right,
    /// Both endpoints are included (closed interval).
    Both,
    /// Neither endpoint is included (open interval).
    Neither,
}

impl Serialize for RangeClosed {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(match self {
            RangeClosed::Left => "left",
            RangeClosed::Right => "right",
            RangeClosed::Both => "both",
            RangeClosed::Neither => "neither",
        })
    }
}

struct RangeClosedVisitor;

impl<'de> Visitor<'de> for RangeClosedVisitor {
    type Value = RangeClosed;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("one of \"left\", \"right\", \"both\", \"neither\"")
    }

    fn visit_str<E>(self, value: &str) -> Result<RangeClosed, E>
    where
        E: serde_core::de::Error,
    {
        match value {
            "left" => Ok(RangeClosed::Left),
            "right" => Ok(RangeClosed::Right),
            "both" => Ok(RangeClosed::Both),
            "neither" => Ok(RangeClosed::Neither),
            _ => Err(serde_core::de::Error::unknown_variant(
                value,
                &["left", "right", "both", "neither"],
            )),
        }
    }
}

impl<'de> Deserialize<'de> for RangeClosed {
    fn deserialize<D>(deserializer: D) -> Result<RangeClosed, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(RangeClosedVisitor)
    }
}

/// Extension type metadata for [`Range`].
#[derive(Debug, Clone, PartialEq)]
pub struct RangeMetadata {
    /// Whether the interval endpoints are included or excluded.
    closed: RangeClosed,
}

impl RangeMetadata {
    /// Returns a new `RangeMetadata`.
    pub fn new(closed: RangeClosed) -> Self {
        RangeMetadata { closed }
    }

    /// Returns whether the interval endpoints are included or excluded.
    pub fn closed(&self) -> RangeClosed {
        self.closed
    }
}

impl Serialize for RangeMetadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("RangeMetadata", 1)?;
        state.serialize_field("closed", &self.closed)?;
        state.end()
    }
}

#[derive(Debug)]
enum MetadataField {
    Closed,
}

struct MetadataFieldVisitor;

impl<'de> Visitor<'de> for MetadataFieldVisitor {
    type Value = MetadataField;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("`closed`")
    }

    fn visit_str<E>(self, value: &str) -> Result<MetadataField, E>
    where
        E: serde_core::de::Error,
    {
        match value {
            "closed" => Ok(MetadataField::Closed),
            _ => Err(serde_core::de::Error::unknown_field(value, &["closed"])),
        }
    }
}

impl<'de> Deserialize<'de> for MetadataField {
    fn deserialize<D>(deserializer: D) -> Result<MetadataField, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_identifier(MetadataFieldVisitor)
    }
}

struct RangeMetadataVisitor;

impl<'de> Visitor<'de> for RangeMetadataVisitor {
    type Value = RangeMetadata;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("struct RangeMetadata")
    }

    fn visit_seq<V>(self, mut seq: V) -> Result<RangeMetadata, V::Error>
    where
        V: serde_core::de::SeqAccess<'de>,
    {
        let closed = seq
            .next_element()?
            .ok_or_else(|| serde_core::de::Error::invalid_length(0, &self))?;
        Ok(RangeMetadata { closed })
    }

    fn visit_map<V>(self, mut map: V) -> Result<RangeMetadata, V::Error>
    where
        V: MapAccess<'de>,
    {
        let mut closed = None;

        while let Some(key) = map.next_key()? {
            match key {
                MetadataField::Closed => {
                    if closed.is_some() {
                        return Err(serde_core::de::Error::duplicate_field("closed"));
                    }
                    closed = Some(map.next_value()?);
                }
            }
        }

        let closed = closed.ok_or_else(|| serde_core::de::Error::missing_field("closed"))?;
        Ok(RangeMetadata { closed })
    }
}

impl<'de> Deserialize<'de> for RangeMetadata {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_struct("RangeMetadata", &["closed"], RangeMetadataVisitor)
    }
}

/// The extension type for `Range`.
///
/// Extension name: `arrow.range`.
///
/// A bounded set (mathematical interval) over an orderable value type T.
///
/// The storage type is a `Struct` with exactly two fields:
/// - `lower` (type T): the lower bound. Each bound may independently be
///   nullable or non-nullable. A nullable bound can hold null to represent an
///   unbounded (infinite, exclusive) endpoint. A non-nullable bound is always
///   finite.
/// - `upper` (type T): the upper bound. Same nullability semantics as `lower`.
///
/// Both fields must have the same data type T. The `closed` parameter specifies
/// which endpoints are included.
///
/// <https://arrow.apache.org/docs/format/CanonicalExtensions.html#range>
#[derive(Debug, Clone, PartialEq)]
pub struct Range(RangeMetadata);

impl Range {
    /// Returns a new `Range` extension type.
    pub fn new(closed: RangeClosed) -> Self {
        Self(RangeMetadata::new(closed))
    }

    /// Returns whether the interval endpoints are included or excluded.
    pub fn closed(&self) -> RangeClosed {
        self.0.closed()
    }
}

impl From<RangeMetadata> for Range {
    fn from(value: RangeMetadata) -> Self {
        Self(value)
    }
}

/// Validates that `data_type` is an acceptable storage type for `arrow.range`.
///
/// Checks that the data type is a struct with exactly two fields named
/// "lower" and "upper", both sharing the same data type. Each bound may be
/// nullable or non-nullable independently; nullability is not required.
fn validate_storage(data_type: &DataType) -> Result<(), ArrowError> {
    let fields: &Fields = match data_type {
        DataType::Struct(fields) => fields,
        other => {
            return Err(ArrowError::InvalidArgumentError(format!(
                "Range data type mismatch, expected Struct, found {other}"
            )));
        }
    };

    if fields.len() != 2 {
        return Err(ArrowError::InvalidArgumentError(format!(
            "Range data type mismatch, expected Struct with 2 fields, found {} field(s)",
            fields.len()
        )));
    }

    let lower = &fields[0];
    let upper = &fields[1];

    if lower.name() != "lower" {
        return Err(ArrowError::InvalidArgumentError(format!(
            "Range data type mismatch, expected first field named \"lower\", found \"{}\"",
            lower.name()
        )));
    }

    if upper.name() != "upper" {
        return Err(ArrowError::InvalidArgumentError(format!(
            "Range data type mismatch, expected second field named \"upper\", found \"{}\"",
            upper.name()
        )));
    }

    if lower.data_type() != upper.data_type() {
        return Err(ArrowError::InvalidArgumentError(format!(
            "Range data type mismatch, \"lower\" and \"upper\" fields must have the same data type, found \"{}\" and \"{}\"",
            lower.data_type(),
            upper.data_type()
        )));
    }

    Ok(())
}

impl ExtensionType for Range {
    const NAME: &'static str = "arrow.range";

    type Metadata = RangeMetadata;

    fn metadata(&self) -> &Self::Metadata {
        &self.0
    }

    fn serialize_metadata(&self) -> Option<String> {
        Some(serde_json::to_string(self.metadata()).expect("metadata serialization"))
    }

    fn deserialize_metadata(metadata: Option<&str>) -> Result<Self::Metadata, ArrowError> {
        metadata.map_or_else(
            || {
                Err(ArrowError::InvalidArgumentError(
                    "Range extension type requires metadata".to_owned(),
                ))
            },
            |value| {
                serde_json::from_str(value).map_err(|e| {
                    ArrowError::InvalidArgumentError(format!(
                        "Range metadata deserialization failed: {e}"
                    ))
                })
            },
        )
    }

    fn supports_data_type(&self, data_type: &DataType) -> Result<(), ArrowError> {
        validate_storage(data_type)
    }

    fn try_new(data_type: &DataType, metadata: Self::Metadata) -> Result<Self, ArrowError> {
        validate_storage(data_type)?;
        Ok(Self::from(metadata))
    }

    fn validate(data_type: &DataType, _metadata: Self::Metadata) -> Result<(), ArrowError> {
        validate_storage(data_type)
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "canonical_extension_types")]
    use crate::extension::CanonicalExtensionType;
    use crate::{
        Field,
        extension::{EXTENSION_TYPE_METADATA_KEY, EXTENSION_TYPE_NAME_KEY},
    };

    use super::*;

    fn make_range_struct(value_type: DataType) -> DataType {
        DataType::Struct(
            [
                Field::new("lower", value_type.clone(), true),
                Field::new("upper", value_type, true),
            ]
            .into_iter()
            .collect(),
        )
    }

    #[test]
    fn valid() -> Result<(), ArrowError> {
        let range = Range::new(RangeClosed::Both);
        let storage = make_range_struct(DataType::Int32);
        let mut field = Field::new("", storage, false);
        field.try_with_extension_type(range.clone())?;
        assert_eq!(field.try_extension_type::<Range>()?, range);
        #[cfg(feature = "canonical_extension_types")]
        assert_eq!(
            field.try_canonical_extension_type()?,
            CanonicalExtensionType::Range(range)
        );
        Ok(())
    }

    #[test]
    fn roundtrip_all_closed_values() -> Result<(), ArrowError> {
        let storage = make_range_struct(DataType::Int32);
        for closed in [
            RangeClosed::Left,
            RangeClosed::Right,
            RangeClosed::Both,
            RangeClosed::Neither,
        ] {
            let range = Range::new(closed);
            let mut field = Field::new("", storage.clone(), false);
            field.try_with_extension_type(range.clone())?;
            let recovered = field.try_extension_type::<Range>()?;
            assert_eq!(recovered.closed(), closed);
        }
        Ok(())
    }

    #[test]
    #[should_panic(expected = "Extension type name missing")]
    fn missing_name() {
        let storage = make_range_struct(DataType::Int32);
        let field = Field::new("", storage, false).with_metadata(
            [(
                EXTENSION_TYPE_METADATA_KEY.to_owned(),
                r#"{"closed":"both"}"#.to_owned(),
            )]
            .into_iter()
            .collect(),
        );
        field.extension_type::<Range>();
    }

    #[test]
    #[should_panic(expected = "Range extension type requires metadata")]
    fn missing_metadata() {
        let storage = make_range_struct(DataType::Int32);
        let field = Field::new("", storage, false).with_metadata(
            [(EXTENSION_TYPE_NAME_KEY.to_owned(), Range::NAME.to_owned())]
                .into_iter()
                .collect(),
        );
        field.extension_type::<Range>();
    }

    #[test]
    #[should_panic(expected = "Range metadata deserialization failed")]
    fn invalid_metadata_bad_closed_string() {
        let storage = make_range_struct(DataType::Int32);
        let field = Field::new("", storage, false).with_metadata(
            [
                (EXTENSION_TYPE_NAME_KEY.to_owned(), Range::NAME.to_owned()),
                (
                    EXTENSION_TYPE_METADATA_KEY.to_owned(),
                    r#"{"closed":"invalid"}"#.to_owned(),
                ),
            ]
            .into_iter()
            .collect(),
        );
        field.extension_type::<Range>();
    }

    #[test]
    #[should_panic(expected = "Range metadata deserialization failed")]
    fn invalid_metadata_missing_closed_key() {
        let storage = make_range_struct(DataType::Int32);
        let field = Field::new("", storage, false).with_metadata(
            [
                (EXTENSION_TYPE_NAME_KEY.to_owned(), Range::NAME.to_owned()),
                (EXTENSION_TYPE_METADATA_KEY.to_owned(), r#"{}"#.to_owned()),
            ]
            .into_iter()
            .collect(),
        );
        field.extension_type::<Range>();
    }

    #[test]
    #[should_panic(expected = "Range data type mismatch, expected Struct, found Int32")]
    fn invalid_storage_non_struct() {
        let range = Range::new(RangeClosed::Both);
        let field = Field::new("", DataType::Int32, false);
        field.with_extension_type(range);
    }

    #[test]
    #[should_panic(expected = "Range data type mismatch, expected Struct with 2 fields")]
    fn invalid_storage_wrong_field_count() {
        let range = Range::new(RangeClosed::Both);
        let storage = DataType::Struct(
            [Field::new("lower", DataType::Int32, true)]
                .into_iter()
                .collect(),
        );
        let field = Field::new("", storage, false);
        field.with_extension_type(range);
    }

    #[test]
    #[should_panic(expected = "Range data type mismatch, expected first field named \"lower\"")]
    fn invalid_storage_wrong_field_names() {
        let range = Range::new(RangeClosed::Both);
        let storage = DataType::Struct(
            [
                Field::new("start", DataType::Int32, true),
                Field::new("end", DataType::Int32, true),
            ]
            .into_iter()
            .collect(),
        );
        let field = Field::new("", storage, false);
        field.with_extension_type(range);
    }

    #[test]
    fn accepts_non_nullable_lower() -> Result<(), ArrowError> {
        let range = Range::new(RangeClosed::Both);
        let storage = DataType::Struct(
            [
                Field::new("lower", DataType::Int32, false),
                Field::new("upper", DataType::Int32, true),
            ]
            .into_iter()
            .collect(),
        );
        let mut field = Field::new("", storage, false);
        field.try_with_extension_type(range.clone())?;
        assert_eq!(field.try_extension_type::<Range>()?, range);
        Ok(())
    }

    #[test]
    fn accepts_non_nullable_upper() -> Result<(), ArrowError> {
        let range = Range::new(RangeClosed::Both);
        let storage = DataType::Struct(
            [
                Field::new("lower", DataType::Int32, true),
                Field::new("upper", DataType::Int32, false),
            ]
            .into_iter()
            .collect(),
        );
        let mut field = Field::new("", storage, false);
        field.try_with_extension_type(range.clone())?;
        assert_eq!(field.try_extension_type::<Range>()?, range);
        Ok(())
    }

    #[test]
    fn accepts_both_non_nullable() -> Result<(), ArrowError> {
        let range = Range::new(RangeClosed::Both);
        let storage = DataType::Struct(
            [
                Field::new("lower", DataType::Int32, false),
                Field::new("upper", DataType::Int32, false),
            ]
            .into_iter()
            .collect(),
        );
        let mut field = Field::new("", storage, false);
        field.try_with_extension_type(range.clone())?;
        assert_eq!(field.try_extension_type::<Range>()?, range);
        Ok(())
    }

    #[test]
    #[should_panic(
        expected = "Range data type mismatch, \"lower\" and \"upper\" fields must have the same data type"
    )]
    fn invalid_storage_mismatched_types() {
        let range = Range::new(RangeClosed::Both);
        let storage = DataType::Struct(
            [
                Field::new("lower", DataType::Int32, true),
                Field::new("upper", DataType::Int64, true),
            ]
            .into_iter()
            .collect(),
        );
        let field = Field::new("", storage, false);
        field.with_extension_type(range);
    }
}
