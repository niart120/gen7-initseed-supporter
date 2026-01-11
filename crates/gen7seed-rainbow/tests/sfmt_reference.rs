use gen7seed_rainbow::Sfmt;

const REF64: &str = include_str!("data/SFMT.19937.64.out.txt");

fn parse_reference_values() -> Vec<u64> {
    let mut in_init_gen_rand = false;
    let mut values = Vec::new();

    for line in REF64.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.contains("init_gen_rand") {
            in_init_gen_rand = true;
            continue;
        }
        if line.contains("init_by_array") {
            break;
        }
        if !in_init_gen_rand {
            continue;
        }
        values.extend(line.split_whitespace().map(|v| v.parse::<u64>().expect("parse reference value")));
    }

    values
}

#[test]
fn sfmt_matches_official_19937_64_reference() {
    let reference = parse_reference_values();
    assert!(reference.len() >= 1000, "reference length: {}", reference.len());

    let mut rng = Sfmt::new(4321);
    for (i, expected) in reference.iter().enumerate().take(1000) {
        let actual = rng.gen_rand_u64();
        assert_eq!(actual, *expected, "mismatch at index {}", i);
    }
}
