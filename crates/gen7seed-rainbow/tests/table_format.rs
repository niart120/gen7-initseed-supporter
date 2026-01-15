use gen7seed_rainbow::constants::{FILE_FORMAT_VERSION, FILE_HEADER_SIZE};
use gen7seed_rainbow::domain::table_format::{
    TableFormatError, TableHeader, ValidationOptions, validate_header,
};

#[test]
fn test_table_header_serialization() {
    let header = TableHeader::new(417, true);
    let bytes = header.to_bytes();
    let restored = TableHeader::from_bytes(&bytes).unwrap();

    assert_eq!(header, restored);
}

#[test]
fn test_table_header_magic_validation() {
    let mut bytes = [0u8; FILE_HEADER_SIZE];
    bytes[0..8].copy_from_slice(b"INVALID\x00");

    let result = TableHeader::from_bytes(&bytes);
    assert!(matches!(result, Err(TableFormatError::InvalidMagic)));
}

#[test]
fn test_table_header_version_validation() {
    let mut header = TableHeader::new(417, true);
    header.version = FILE_FORMAT_VERSION + 1;
    let bytes = header.to_bytes();

    let result = TableHeader::from_bytes(&bytes);
    assert!(matches!(
        result,
        Err(TableFormatError::UnsupportedVersion(_))
    ));
}

#[test]
fn test_validate_consumption_mismatch() {
    let header = TableHeader::new(417, true);
    let options = ValidationOptions::for_search(477);

    let result = validate_header(&header, &options);
    assert!(matches!(
        result,
        Err(TableFormatError::ConsumptionMismatch {
            expected: 477,
            found: 417
        })
    ));
}

#[test]
fn test_validate_chain_length_mismatch() {
    let mut header = TableHeader::new(417, true);
    header.chain_length += 1;
    let options = ValidationOptions::for_search(417);

    let result = validate_header(&header, &options);
    assert!(matches!(
        result,
        Err(TableFormatError::ChainLengthMismatch { .. })
    ));
}
