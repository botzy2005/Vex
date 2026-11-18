//! Conservative detection of text that is probably a secret, so it is never recorded.
//! The patterns are specific (known token prefixes, PEM keys, Luhn-valid card numbers, `password=…`
//! lines) rather than entropy guesses, to keep ordinary copies from being skipped.

fn single_token(s: &str) -> bool {
    !s.is_empty() && !s.chars().any(char::is_whitespace)
}

fn all_of(s: &str, ok: impl Fn(char) -> bool) -> bool {
    s.chars().all(ok)
}

fn luhn(digits: &[u32]) -> bool {
    let mut sum = 0;
    for (i, d) in digits.iter().rev().enumerate() {
        let mut v = *d;
        if i % 2 == 1 {
            v *= 2;
            if v > 9 {
                v -= 9;
            }
        }
        sum += v;
    }
    sum % 10 == 0
}

fn looks_like_card(t: &str) -> bool {
    if !t.chars().all(|c| c.is_ascii_digit() || c == ' ' || c == '-') {
        return false;
    }
    let digits: Vec<u32> = t.chars().filter_map(|c| c.to_digit(10)).collect();
    (13..=19).contains(&digits.len()) && luhn(&digits) && digits.iter().any(|&d| d != 0)
}

fn looks_like_token(t: &str) -> bool {
    let url_safe = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+' | '/' | '=');
    if !single_token(t) || !all_of(t, url_safe) {
        return false;
    }
    let prefixed = [
        ("sk-", 20),
        ("sk_live_", 20),
        ("rk_live_", 20),
        ("ghp_", 30),
        ("gho_", 30),
        ("ghu_", 30),
        ("ghs_", 30),
        ("ghr_", 30),
        ("github_pat_", 30),
        ("glpat-", 20),
        ("xoxb-", 20),
        ("xoxp-", 20),
        ("xoxa-", 20),
        ("xoxs-", 20),
        ("hf_", 30),
        ("npm_", 36),
        ("AIza", 35),
    ];
    if prefixed.iter().any(|(p, min)| t.starts_with(p) && t.len() >= *min) {
        return true;
    }
    // AWS access key id.
    if t.len() == 20 && (t.starts_with("AKIA") || t.starts_with("ASIA")) && all_of(t, |c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
        return true;
    }
    // JSON web token: three dot-separated base64url parts, the first starting with a `{"` header.
    let parts: Vec<&str> = t.split('.').collect();
    parts.len() == 3 && parts[0].starts_with("eyJ") && parts.iter().all(|p| p.len() >= 8) && t.len() >= 40
}

fn names_a_secret(line: &str) -> bool {
    let lower = line.to_lowercase();
    ["password", "passwd", "pwd", "secret", "api_key", "api-key", "apikey", "token", "private_key", "access_key"].iter().any(|k| {
        lower.find(k).is_some_and(|at| {
            let rest = lower[at + k.len()..].trim_start_matches(|c: char| c.is_alphanumeric() || c == '_' || c == '-');
            let rest = rest.trim_start();
            (rest.starts_with(':') || rest.starts_with('=')) && rest[1..].trim().chars().count() >= 4
        })
    })
}

/// True when `text` should not be recorded.
pub fn looks_sensitive(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() {
        return false;
    }
    if t.contains("-----BEGIN ") && t.contains("PRIVATE KEY-----") {
        return true;
    }
    if t.chars().count() > 4000 {
        return false;
    }
    if looks_like_card(t) || looks_like_token(t) {
        return true;
    }
    // "password: hunter2" style lines, only for short single-line copies.
    t.chars().count() <= 200 && !t.contains('\n') && names_a_secret(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_secret_shapes_are_caught() {
        for s in [
            concat!("s", "k-abcdefghijklmnopqrstuvwxyz123456"),
            concat!("gh", "p_abcdefghijklmnopqrstuvwxyz0123456789"),
            concat!("github", "_pat_11ABCDEFG0123456789_abcdefghijklmnopqrstuv"),
            concat!("AK", "IAIOSFODNN7EXAMPLE"),
            concat!("xo", "xb-1234567890-abcdefghijklmnop"),
            concat!("AI", "zaSyA-abcdefghijklmnopqrstuvwxyz01234"),
            concat!("ey", "JhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dBjftJeZ4CVPmB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            concat!("-----BEGIN RSA ", "PRIVATE KEY-----\nMIIE...\n-----END RSA PRIVATE KEY-----"),
            "  4111 1111 1111 1111  ",
            "4111-1111-1111-1111",
            "password: hunter2!",
            "export API_KEY=abc12345",
            "DB_PASSWORD = s3cr3tvalue",
        ] {
            assert!(looks_sensitive(s), "should be sensitive: {s}");
        }
    }

    #[test]
    fn ordinary_copies_are_kept() {
        for s in [
            "Meeting at 3pm in room 4",
            "https://example.com/some/long/path?with=query&and=more",
            "C:\\Users\\me\\Documents\\report-final-v2.docx",
            "The password policy requires twelve characters.",
            "Reset your password",
            "token",
            "4111 1111 1111 1112", // fails Luhn
            "1234567890123",       // 13 digits but fails Luhn
            "0000 0000 0000 0000",
            "fn main() { println!(\"hi\"); }",
            "sk-short",
            "a3f9c2d1e8b74a6f9d0c1b2a3e4f5a6b", // a plain hash
            "2024-11-18",
            "",
            "   ",
        ] {
            assert!(!looks_sensitive(s), "should be kept: {s}");
        }
    }

    #[test]
    fn long_documents_are_not_scanned_for_loose_patterns_but_private_keys_still_are() {
        let long = format!("{}\npassword: hunter2", "lorem ipsum ".repeat(500));
        assert!(!looks_sensitive(&long));
        let key = format!("{}-----BEGIN {}-----\nabc\n-----END PRIVATE KEY-----", "x\n".repeat(3000), "PRIVATE KEY");
        assert!(looks_sensitive(&key));
    }
}
