//! Shared test/bench helpers: deterministic log generator.
//! Available to unit tests, integration tests and benches.

/// Generate a synthetic log of `lines` lines, deterministic via a simple
/// linear-congruential PRNG (fixed seed). Each line resembles a HIL bench log.
pub fn gen_log_lines(lines: usize) -> String {
    let mut state: u64 = 0x1234_5678_9abc_def0;
    let mut out = String::with_capacity(lines * 48);
    for i in 0..lines {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let v = (state >> 33) % 1000;
        out.push_str(&format!(
            "2026-10-24 12:00:{:02}.{:03} INFO  [task-{}] ECUsync ok val={}\n",
            i % 60,
            v,
            i % 8,
            v
        ));
    }
    out
}

/// Bytes of a synthetic log sized to ~`size_mb` (approximate, ASCII lines).
pub fn gen_log(size_mb: usize) -> Vec<u8> {
    let target = size_mb * 1024 * 1024;
    let per_line = 64usize;
    gen_log_lines(target / per_line).into_bytes()
}

#[cfg(test)]
mod tests {
    use super::{gen_log, gen_log_lines};

    #[test]
    fn zero_lines_produces_no_output() {
        assert_eq!(gen_log_lines(0), "");
        assert!(gen_log(0).is_empty());
    }

    #[test]
    fn line_count_and_shape_match_the_request() {
        let text = gen_log_lines(3);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(text.ends_with('\n'));
        for (i, line) in lines.iter().enumerate() {
            assert!(line.starts_with("2026-10-24 12:00:"), "line {i}: {line}");
            assert!(line.contains(" INFO  [task-"), "line {i}: {line}");
            assert!(line.contains("] ECUsync ok val="), "line {i}: {line}");
        }
        assert!(lines[0].contains("[task-0]"));
        assert!(lines[1].contains("[task-1]"));
    }

    #[test]
    fn generator_is_deterministic_and_prefix_stable() {
        let a = gen_log_lines(8);
        assert_eq!(a, gen_log_lines(8));
        let b = gen_log_lines(3);
        assert!(a.starts_with(&b));
    }

    #[test]
    fn generated_bytes_are_utf8_and_match_the_line_generator() {
        let bytes = gen_log(1);
        assert_eq!(bytes, gen_log_lines(1024 * 1024 / 64).into_bytes());
        assert!(bytes.len() > 512 * 1024 && bytes.len() < 2 * 1024 * 1024);
        let text = std::str::from_utf8(&bytes).expect("synthetic log is ASCII");
        assert!(text.ends_with('\n'));
        assert_eq!(text.lines().count(), 1024 * 1024 / 64);
    }
}
