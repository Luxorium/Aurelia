use std::collections::BTreeMap;
use std::io::{self, Read, Write};

pub const TAG_END: u8 = 0;
pub const TAG_BYTE: u8 = 1;
pub const TAG_SHORT: u8 = 2;
pub const TAG_INT: u8 = 3;
pub const TAG_LONG: u8 = 4;
pub const TAG_FLOAT: u8 = 5;
pub const TAG_DOUBLE: u8 = 6;
pub const TAG_BYTE_ARRAY: u8 = 7;
pub const TAG_STRING: u8 = 8;
pub const TAG_LIST: u8 = 9;
pub const TAG_COMPOUND: u8 = 10;
pub const TAG_INT_ARRAY: u8 = 11;

pub type Compound = BTreeMap<String, Tag>;

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub root_name: String,
    pub root: Compound,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<u8>),
    String(String),
    List {
        element_type: u8,
        elements: Vec<Tag>,
    },
    Compound(Compound),
    IntArray(Vec<i32>),
}

impl Tag {
    pub const fn tag_id(&self) -> u8 {
        match self {
            Self::Byte(_) => TAG_BYTE,
            Self::Short(_) => TAG_SHORT,
            Self::Int(_) => TAG_INT,
            Self::Long(_) => TAG_LONG,
            Self::Float(_) => TAG_FLOAT,
            Self::Double(_) => TAG_DOUBLE,
            Self::ByteArray(_) => TAG_BYTE_ARRAY,
            Self::String(_) => TAG_STRING,
            Self::List { .. } => TAG_LIST,
            Self::Compound(_) => TAG_COMPOUND,
            Self::IntArray(_) => TAG_INT_ARRAY,
        }
    }

    pub fn as_compound(&self) -> Option<&Compound> {
        match self {
            Self::Compound(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_compound_mut(&mut self) -> Option<&mut Compound> {
        match self {
            Self::Compound(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_byte_array(&self) -> Option<&[u8]> {
        match self {
            Self::ByteArray(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<(u8, &[Tag])> {
        match self {
            Self::List {
                element_type,
                elements,
            } => Some((*element_type, elements)),
            _ => None,
        }
    }

    pub fn as_i32(&self) -> Option<i32> {
        match self {
            Self::Int(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Long(value) => Some(*value),
            Self::Int(value) => Some(i64::from(*value)),
            _ => None,
        }
    }

    pub fn as_f32(&self) -> Option<f32> {
        match self {
            Self::Float(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Double(value) => Some(*value),
            Self::Float(value) => Some(f64::from(*value)),
            _ => None,
        }
    }

    pub fn as_i8(&self) -> Option<i8> {
        match self {
            Self::Byte(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_i16(&self) -> Option<i16> {
        match self {
            Self::Short(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_int_array(&self) -> Option<&[i32]> {
        match self {
            Self::IntArray(values) => Some(values),
            _ => None,
        }
    }
}

pub fn read_document(input: &mut impl Read) -> io::Result<Document> {
    let tag_id = read_u8(input)?;
    if tag_id != TAG_COMPOUND {
        return Err(invalid_data("NBT root tag must be a named compound"));
    }
    let root_name = read_string(input)?;
    let root = read_compound_payload(input)?;
    Ok(Document { root_name, root })
}

pub fn write_document(document: &Document, output: &mut impl Write) -> io::Result<()> {
    write_u8(output, TAG_COMPOUND)?;
    write_string(output, &document.root_name)?;
    write_compound_payload(output, &document.root)
}

fn read_tag_payload(input: &mut impl Read, tag_id: u8) -> io::Result<Tag> {
    match tag_id {
        TAG_BYTE => Ok(Tag::Byte(read_i8(input)?)),
        TAG_SHORT => Ok(Tag::Short(read_i16(input)?)),
        TAG_INT => Ok(Tag::Int(read_i32(input)?)),
        TAG_LONG => Ok(Tag::Long(read_i64(input)?)),
        TAG_FLOAT => Ok(Tag::Float(read_f32(input)?)),
        TAG_DOUBLE => Ok(Tag::Double(read_f64(input)?)),
        TAG_BYTE_ARRAY => {
            let len = read_i32(input)?;
            if len < 0 {
                return Err(invalid_data("negative NBT byte array length"));
            }
            let mut bytes = vec![0; len as usize];
            input.read_exact(&mut bytes)?;
            Ok(Tag::ByteArray(bytes))
        }
        TAG_STRING => Ok(Tag::String(read_string(input)?)),
        TAG_LIST => {
            let element_type = read_u8(input)?;
            let len = read_i32(input)?;
            if len < 0 {
                return Err(invalid_data("negative NBT list length"));
            }
            let mut elements = Vec::with_capacity(len as usize);
            for _ in 0..len {
                elements.push(read_tag_payload(input, element_type)?);
            }
            Ok(Tag::List {
                element_type,
                elements,
            })
        }
        TAG_COMPOUND => Ok(Tag::Compound(read_compound_payload(input)?)),
        TAG_INT_ARRAY => {
            let len = read_i32(input)?;
            if len < 0 {
                return Err(invalid_data("negative NBT int array length"));
            }
            let mut values = Vec::with_capacity(len as usize);
            for _ in 0..len {
                values.push(read_i32(input)?);
            }
            Ok(Tag::IntArray(values))
        }
        TAG_END => Err(invalid_data("unexpected TAG_End payload")),
        _ => Err(invalid_data(format!("unsupported NBT tag id {tag_id}"))),
    }
}

fn write_tag_payload(output: &mut impl Write, tag: &Tag) -> io::Result<()> {
    match tag {
        Tag::Byte(value) => write_i8(output, *value),
        Tag::Short(value) => write_i16(output, *value),
        Tag::Int(value) => write_i32(output, *value),
        Tag::Long(value) => write_i64(output, *value),
        Tag::Float(value) => write_f32(output, *value),
        Tag::Double(value) => write_f64(output, *value),
        Tag::ByteArray(bytes) => {
            write_i32(output, checked_len(bytes.len(), "NBT byte array")?)?;
            output.write_all(bytes)
        }
        Tag::String(value) => write_string(output, value),
        Tag::List {
            element_type,
            elements,
        } => {
            for element in elements {
                if element.tag_id() != *element_type {
                    return Err(invalid_data("NBT list element type mismatch"));
                }
            }
            write_u8(output, *element_type)?;
            write_i32(output, checked_len(elements.len(), "NBT list")?)?;
            for element in elements {
                write_tag_payload(output, element)?;
            }
            Ok(())
        }
        Tag::Compound(value) => write_compound_payload(output, value),
        Tag::IntArray(values) => {
            write_i32(output, checked_len(values.len(), "NBT int array")?)?;
            for value in values {
                write_i32(output, *value)?;
            }
            Ok(())
        }
    }
}

fn read_compound_payload(input: &mut impl Read) -> io::Result<Compound> {
    let mut compound = Compound::new();
    loop {
        let tag_id = read_u8(input)?;
        if tag_id == TAG_END {
            break;
        }
        let name = read_string(input)?;
        let tag = read_tag_payload(input, tag_id)?;
        compound.insert(name, tag);
    }
    Ok(compound)
}

fn write_compound_payload(output: &mut impl Write, compound: &Compound) -> io::Result<()> {
    for (name, tag) in compound {
        write_u8(output, tag.tag_id())?;
        write_string(output, name)?;
        write_tag_payload(output, tag)?;
    }
    write_u8(output, TAG_END)
}

fn read_string(input: &mut impl Read) -> io::Result<String> {
    let len = read_u16(input)? as usize;
    let mut bytes = vec![0; len];
    input.read_exact(&mut bytes)?;
    String::from_utf8(bytes).map_err(|_| invalid_data("NBT string is not valid UTF-8"))
}

fn write_string(output: &mut impl Write, value: &str) -> io::Result<()> {
    let bytes = value.as_bytes();
    if bytes.len() > u16::MAX as usize {
        return Err(invalid_data("NBT string is too long"));
    }
    output.write_all(&(bytes.len() as u16).to_be_bytes())?;
    output.write_all(bytes)
}

fn checked_len(len: usize, label: &str) -> io::Result<i32> {
    i32::try_from(len).map_err(|_| invalid_data(format!("{label} length exceeds i32::MAX")))
}

fn read_u8(input: &mut impl Read) -> io::Result<u8> {
    let mut bytes = [0; 1];
    input.read_exact(&mut bytes)?;
    Ok(bytes[0])
}

fn read_i8(input: &mut impl Read) -> io::Result<i8> {
    Ok(read_u8(input)? as i8)
}

fn read_u16(input: &mut impl Read) -> io::Result<u16> {
    let mut bytes = [0; 2];
    input.read_exact(&mut bytes)?;
    Ok(u16::from_be_bytes(bytes))
}

fn read_i16(input: &mut impl Read) -> io::Result<i16> {
    let mut bytes = [0; 2];
    input.read_exact(&mut bytes)?;
    Ok(i16::from_be_bytes(bytes))
}

fn read_i32(input: &mut impl Read) -> io::Result<i32> {
    let mut bytes = [0; 4];
    input.read_exact(&mut bytes)?;
    Ok(i32::from_be_bytes(bytes))
}

fn read_i64(input: &mut impl Read) -> io::Result<i64> {
    let mut bytes = [0; 8];
    input.read_exact(&mut bytes)?;
    Ok(i64::from_be_bytes(bytes))
}

fn read_f32(input: &mut impl Read) -> io::Result<f32> {
    Ok(f32::from_bits(read_i32(input)? as u32))
}

fn read_f64(input: &mut impl Read) -> io::Result<f64> {
    Ok(f64::from_bits(read_i64(input)? as u64))
}

fn write_u8(output: &mut impl Write, value: u8) -> io::Result<()> {
    output.write_all(&[value])
}

fn write_i8(output: &mut impl Write, value: i8) -> io::Result<()> {
    output.write_all(&[value as u8])
}

fn write_i16(output: &mut impl Write, value: i16) -> io::Result<()> {
    output.write_all(&value.to_be_bytes())
}

fn write_i32(output: &mut impl Write, value: i32) -> io::Result<()> {
    output.write_all(&value.to_be_bytes())
}

fn write_i64(output: &mut impl Write, value: i64) -> io::Result<()> {
    output.write_all(&value.to_be_bytes())
}

fn write_f32(output: &mut impl Write, value: f32) -> io::Result<()> {
    output.write_all(&value.to_bits().to_be_bytes())
}

fn write_f64(output: &mut impl Write, value: f64) -> io::Result<()> {
    output.write_all(&value.to_bits().to_be_bytes())
}

fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(document: Document) -> Document {
        let mut encoded = Vec::new();
        write_document(&document, &mut encoded).unwrap();
        read_document(&mut encoded.as_slice()).unwrap()
    }

    fn simple_doc(root: Compound) -> Document {
        Document {
            root_name: String::new(),
            root,
        }
    }

    fn single(name: &str, tag: Tag) -> Compound {
        let mut c = Compound::new();
        c.insert(name.to_string(), tag);
        c
    }

    #[test]
    fn nbt_document_round_trips_required_beta_tags() {
        let mut nested = Compound::new();
        nested.insert("flag".to_string(), Tag::Byte(1));

        let mut root = Compound::new();
        root.insert("byte".to_string(), Tag::Byte(-2));
        root.insert("short".to_string(), Tag::Short(32000));
        root.insert("int".to_string(), Tag::Int(123456));
        root.insert("long".to_string(), Tag::Long(9_000_000_000));
        root.insert("float".to_string(), Tag::Float(1.25));
        root.insert("double".to_string(), Tag::Double(2.5));
        root.insert("bytes".to_string(), Tag::ByteArray(vec![0, 1, 255]));
        root.insert("string".to_string(), Tag::String("Beta".to_string()));
        root.insert(
            "list".to_string(),
            Tag::List {
                element_type: TAG_INT,
                elements: vec![Tag::Int(1), Tag::Int(2), Tag::Int(3)],
            },
        );
        root.insert("compound".to_string(), Tag::Compound(nested));
        let document = Document {
            root_name: "Root".to_string(),
            root,
        };

        assert_eq!(document, round_trip(document.clone()));
    }

    #[test]
    fn nbt_all_scalar_boundaries_round_trip() {
        let mut root = Compound::new();
        root.insert("i8_min".to_string(), Tag::Byte(i8::MIN));
        root.insert("i8_max".to_string(), Tag::Byte(i8::MAX));
        root.insert("i16_min".to_string(), Tag::Short(i16::MIN));
        root.insert("i16_max".to_string(), Tag::Short(i16::MAX));
        root.insert("i32_min".to_string(), Tag::Int(i32::MIN));
        root.insert("i32_max".to_string(), Tag::Int(i32::MAX));
        root.insert("i64_min".to_string(), Tag::Long(i64::MIN));
        root.insert("i64_max".to_string(), Tag::Long(i64::MAX));
        root.insert("f32_neg".to_string(), Tag::Float(-1.0_f32));
        root.insert("f32_pos".to_string(), Tag::Float(f32::MAX));
        root.insert("f64_neg".to_string(), Tag::Double(f64::NEG_INFINITY));
        root.insert("f64_pos".to_string(), Tag::Double(f64::INFINITY));

        let doc = simple_doc(root);
        assert_eq!(doc, round_trip(doc.clone()));
    }

    #[test]
    fn nbt_int_array_round_trips() {
        let doc = simple_doc(single(
            "arr",
            Tag::IntArray(vec![i32::MIN, -1, 0, 1, i32::MAX]),
        ));
        assert_eq!(doc, round_trip(doc.clone()));
    }

    #[test]
    fn nbt_empty_collections_round_trip() {
        let mut root = Compound::new();
        root.insert("empty_bytes".to_string(), Tag::ByteArray(vec![]));
        root.insert("empty_ints".to_string(), Tag::IntArray(vec![]));
        root.insert(
            "empty_list".to_string(),
            Tag::List {
                element_type: TAG_INT,
                elements: vec![],
            },
        );
        root.insert("empty_compound".to_string(), Tag::Compound(Compound::new()));

        let doc = simple_doc(root);
        assert_eq!(doc, round_trip(doc.clone()));
    }

    #[test]
    fn nbt_list_of_compounds_round_trips() {
        let mut a = Compound::new();
        a.insert("x".to_string(), Tag::Int(1));
        let mut b = Compound::new();
        b.insert("x".to_string(), Tag::Int(2));

        let doc = simple_doc(single(
            "entries",
            Tag::List {
                element_type: TAG_COMPOUND,
                elements: vec![Tag::Compound(a), Tag::Compound(b)],
            },
        ));
        assert_eq!(doc, round_trip(doc.clone()));
    }

    #[test]
    fn nbt_deeply_nested_compound_round_trips() {
        let inner = single("leaf", Tag::Long(42));
        let mid = single("inner", Tag::Compound(inner));
        let outer = single("mid", Tag::Compound(mid));

        let doc = simple_doc(outer);
        assert_eq!(doc, round_trip(doc.clone()));
    }

    #[test]
    fn nbt_empty_string_tag_round_trips() {
        let doc = simple_doc(single("s", Tag::String(String::new())));
        assert_eq!(doc, round_trip(doc.clone()));
    }

    #[test]
    fn nbt_empty_root_name_round_trips() {
        let doc = Document {
            root_name: String::new(),
            root: Compound::new(),
        };
        assert_eq!(doc, round_trip(doc.clone()));
    }

    #[test]
    fn nbt_non_empty_root_name_round_trips() {
        let doc = Document {
            root_name: "MinecraftLevel".to_string(),
            root: Compound::new(),
        };
        assert_eq!(doc, round_trip(doc.clone()));
    }

    #[test]
    fn nbt_rejects_non_compound_root() {
        let bytes = [TAG_BYTE, 0x00, 0x00, 0x05];
        assert!(read_document(&mut bytes.as_ref()).is_err());
    }

    #[test]
    fn nbt_rejects_truncated_input() {
        let bytes = [TAG_COMPOUND];
        assert!(read_document(&mut bytes.as_ref()).is_err());
    }

    #[test]
    fn nbt_rejects_unknown_tag_id() {
        let bytes = [
            TAG_COMPOUND,
            0x00,
            0x00, // root: TAG_COMPOUND, name=""
            99,   // unknown tag id inside compound
        ];
        assert!(read_document(&mut bytes.as_ref()).is_err());
    }

    #[test]
    fn nbt_rejects_negative_byte_array_length() {
        let bytes = [
            TAG_COMPOUND,
            0x00,
            0x00, // root
            TAG_BYTE_ARRAY,
            0x00,
            0x01,
            b'x', // "x": ByteArray
            0xFF,
            0xFF,
            0xFF,
            0xFF, // length = -1
            TAG_END,
        ];
        assert!(read_document(&mut bytes.as_ref()).is_err());
    }

    #[test]
    fn nbt_rejects_negative_int_array_length() {
        let bytes = [
            TAG_COMPOUND,
            0x00,
            0x00, // root
            TAG_INT_ARRAY,
            0x00,
            0x01,
            b'x', // "x": IntArray
            0xFF,
            0xFF,
            0xFF,
            0xFF, // length = -1
            TAG_END,
        ];
        assert!(read_document(&mut bytes.as_ref()).is_err());
    }

    #[test]
    fn nbt_rejects_negative_list_length() {
        let bytes = [
            TAG_COMPOUND,
            0x00,
            0x00, // root
            TAG_LIST,
            0x00,
            0x01,
            b'x',    // "x": List
            TAG_INT, // element type
            0xFF,
            0xFF,
            0xFF,
            0xFF, // length = -1
            TAG_END,
        ];
        assert!(read_document(&mut bytes.as_ref()).is_err());
    }

    #[test]
    fn nbt_rejects_invalid_utf8_in_tag_name() {
        let bytes = [
            TAG_COMPOUND,
            0x00,
            0x00, // root
            TAG_BYTE,
            0x00,
            0x02,
            0xFF,
            0xFE, // name: 2 bytes, invalid UTF-8
            0x01, // byte payload
            TAG_END,
        ];
        assert!(read_document(&mut bytes.as_ref()).is_err());
    }

    #[test]
    fn nbt_rejects_invalid_utf8_in_string_value() {
        let bytes = [
            TAG_COMPOUND,
            0x00,
            0x00, // root
            TAG_STRING,
            0x00,
            0x01,
            b'x', // "x": String
            0x00,
            0x02,
            0xFF,
            0xFE, // string value: 2 bytes, invalid UTF-8
            TAG_END,
        ];
        assert!(read_document(&mut bytes.as_ref()).is_err());
    }

    #[test]
    fn nbt_helper_accessors_return_correct_values_and_none_for_wrong_type() {
        let byte_tag = Tag::Byte(-5);
        let short_tag = Tag::Short(300);
        let int_tag = Tag::Int(70000);
        let long_tag = Tag::Long(5_000_000_000);
        let arr_tag = Tag::IntArray(vec![10, 20]);

        assert_eq!(Some(-5_i8), byte_tag.as_i8());
        assert_eq!(None, int_tag.as_i8());

        assert_eq!(Some(300_i16), short_tag.as_i16());
        assert_eq!(None, byte_tag.as_i16());

        assert_eq!(Some(70000_i32), int_tag.as_i32());
        assert_eq!(None, byte_tag.as_i32());

        assert_eq!(Some(5_000_000_000_i64), long_tag.as_i64());
        assert_eq!(Some(70000_i64), int_tag.as_i64());
        assert_eq!(None, byte_tag.as_i64());

        assert_eq!(Some(&[10_i32, 20][..]), arr_tag.as_int_array());
        assert_eq!(None, int_tag.as_int_array());

        let str_tag = Tag::String("hello".to_string());
        assert_eq!(Some("hello"), str_tag.as_str());
        assert_eq!(None, byte_tag.as_str());
    }

    #[test]
    fn nbt_list_type_mismatch_is_rejected_on_write() {
        let doc = simple_doc(single(
            "bad",
            Tag::List {
                element_type: TAG_INT,
                elements: vec![Tag::Int(1), Tag::Byte(2)],
            },
        ));
        let mut buf = Vec::new();
        assert!(write_document(&doc, &mut buf).is_err());
    }
}
