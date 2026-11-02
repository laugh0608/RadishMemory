//! Deterministic scale fixture; literal version markers are the independent oracle.
pub const CORPUS: &str = "q04-hundred-mixed-v1";
pub const SOURCES: usize = 100;
pub const SHORT_BYTES: usize = 768;
pub const LONG_BYTES: usize = 16_384;
pub const COMMON_QUERY: &str = "合成资料";

pub fn versions(index: usize) -> usize {
    match index % 10 {
        0 | 1 => 3,
        2 | 3 => 2,
        _ => 1,
    }
}

pub fn marker(index: usize, version: usize) -> String {
    format!("q04version{index:03}v{version}")
}

pub fn body(index: usize, version: usize) -> String {
    assert!((1..=versions(index)).contains(&version));
    let target = if index.is_multiple_of(4) {
        LONG_BYTES
    } else {
        SHORT_BYTES
    };
    let mut body = format!("# 合成资料 {index:03}\nAPI UTF-8 本地版本 {version}。\n");
    let tail = format!("\n{}\n", marker(index, version));
    let paragraph = "合成资料保留来源与版本。Local records keep exact bytes.\n";
    while body.len() + paragraph.len() + tail.len() <= target {
        body.push_str(paragraph);
    }
    body.push_str(&" ".repeat(target - body.len() - tail.len()));
    body.push_str(&tail);
    assert_eq!(body.len(), target);
    body
}

#[test]
fn fixed_scale_fixture_pins_sizes_version_distribution_and_literal_markers() {
    let mut distribution = [0; 3];
    let mut long = 0;
    let mut latest_bytes = 0;
    let mut all_bytes = 0;
    for index in 0..SOURCES {
        let count = versions(index);
        distribution[count - 1] += 1;
        if index.is_multiple_of(4) {
            long += 1;
        }
        let current = body(index, count);
        latest_bytes += current.len();
        for version in 1..=count {
            let text = body(index, version);
            assert!(text.contains(COMMON_QUERY));
            assert!(text.contains(&marker(index, version)));
            if version != count {
                assert!(!current.contains(&marker(index, version)));
            }
            all_bytes += text.len();
        }
    }
    assert_eq!(distribution, [60, 20, 20]);
    assert_eq!(long, 25);
    assert_eq!(latest_bytes, 467_200);
    assert_eq!(all_bytes, 747_520);
}
