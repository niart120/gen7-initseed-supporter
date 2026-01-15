use gen7seed_rainbow::constants::FILE_HEADER_SIZE;
use gen7seed_rainbow::domain::missing_format::{
    MissingFormatError, MissingSeedsHeader, calculate_source_checksum,
};
use gen7seed_rainbow::domain::table_format::TableHeader;

#[test]
fn test_missing_header_serialization() {
    let table_header = TableHeader::new(417, true);
    let missing_header = MissingSeedsHeader::new(&table_header, 12345);

    let bytes = missing_header.to_bytes();
    let restored = MissingSeedsHeader::from_bytes(&bytes).unwrap();

    assert_eq!(missing_header, restored);
}

#[test]
fn test_missing_header_magic_validation() {
    let mut bytes = [0u8; FILE_HEADER_SIZE];
    bytes[0..8].copy_from_slice(b"INVALID\x00");

    let result = MissingSeedsHeader::from_bytes(&bytes);
    assert!(matches!(result, Err(MissingFormatError::InvalidMagic)));
}

#[test]
fn test_source_checksum_calculation() {
    let table_header = TableHeader::new(417, true);
    let checksum1 = calculate_source_checksum(&table_header);
    let checksum2 = calculate_source_checksum(&table_header);

    assert_eq!(checksum1, checksum2);
}

#[test]
fn test_source_verification() {
    let table_header = TableHeader::new(417, true);
    let missing_header = MissingSeedsHeader::new(&table_header, 100);

    assert!(missing_header.verify_source(&table_header).is_ok());

    let other_header = TableHeader::new(477, true);
    assert!(matches!(
        missing_header.verify_source(&other_header),
        Err(MissingFormatError::SourceMismatch { .. })
    ));
}
