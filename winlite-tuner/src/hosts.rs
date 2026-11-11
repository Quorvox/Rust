use anyhow::{Result, bail};

const BEGIN: &str = "# winlite-tuner begin";
const END: &str = "# winlite-tuner end";

pub fn has_block(content: &str) -> bool {
    content.lines().any(|l| l.trim() == BEGIN)
}

fn newline(content: &str) -> &'static str {
    if content.contains("\r\n") { "\r\n" } else { "\n" }
}

// вырезает наш блок. начало без конца → ошибка, иначе можно снести пол файла
pub fn remove_block(content: &str) -> Result<String> {
    let lines: Vec<&str> = content.lines().collect();
    let Some(b) = lines.iter().position(|l| l.trim() == BEGIN) else {
        return Ok(content.to_string());
    };
    let Some(e) = lines[b..].iter().position(|l| l.trim() == END) else {
        bail!("в hosts есть начало блока winlite-tuner без конца, поправь руками");
    };
    let e = b + e;

    let mut out: Vec<&str> = lines[..b].to_vec();
    out.extend_from_slice(&lines[e + 1..]);

    let nl = newline(content);
    let mut res = out.join(nl);
    if !out.is_empty() && content.ends_with('\n') {
        res.push_str(nl);
    }
    // вдруг блоков было несколько
    if has_block(&res) { remove_block(&res) } else { Ok(res) }
}

pub fn apply_block(content: &str, domains: &[String]) -> Result<String> {
    let mut res = remove_block(content)?;
    let nl = newline(content);
    // файл мог кончаться без \n: тогда и блок так же, чтобы откат был байт-в-байт
    let had_nl = res.is_empty() || res.ends_with('\n');
    if !had_nl {
        res.push_str(nl);
    }
    res.push_str(BEGIN);
    res.push_str(nl);
    for d in domains {
        res.push_str("0.0.0.0 ");
        res.push_str(d);
        res.push_str(nl);
    }
    res.push_str(END);
    if had_nl {
        res.push_str(nl);
    }
    Ok(res)
}

pub fn parse_domains(raw: &str) -> Vec<String> {
    raw.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doms() -> Vec<String> {
        vec!["a.example".into(), "b.example".into()]
    }

    #[test]
    fn adds_block() {
        let r = apply_block("127.0.0.1 localhost\n", &doms()).unwrap();
        assert!(r.contains("0.0.0.0 a.example"));
        assert!(r.starts_with("127.0.0.1 localhost\n"));
        assert!(has_block(&r));
    }

    #[test]
    fn apply_twice_same_as_once() {
        let once = apply_block("x\n", &doms()).unwrap();
        let twice = apply_block(&once, &doms()).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn roundtrip_returns_original() {
        let orig = "# comment\n127.0.0.1 localhost\n";
        let back = remove_block(&apply_block(orig, &doms()).unwrap()).unwrap();
        assert_eq!(back, orig);
    }

    #[test]
    fn roundtrip_without_trailing_newline() {
        for orig in ["127.0.0.1 localhost", "a\r\nb"] {
            let back = remove_block(&apply_block(orig, &doms()).unwrap()).unwrap();
            assert_eq!(back, orig);
        }
    }

    #[test]
    fn keeps_crlf() {
        let orig = "127.0.0.1 localhost\r\n";
        let r = apply_block(orig, &doms()).unwrap();
        assert!(!r.replace("\r\n", "").contains('\n'));
        assert_eq!(remove_block(&r).unwrap(), orig);
    }

    #[test]
    fn empty_file() {
        let r = apply_block("", &doms()).unwrap();
        assert_eq!(remove_block(&r).unwrap(), "");
    }

    #[test]
    fn begin_without_end_is_error() {
        let broken = format!("keep\n{BEGIN}\n0.0.0.0 a.example\n");
        assert!(remove_block(&broken).is_err());
    }

    #[test]
    fn other_lines_untouched() {
        let orig = "1.1.1.1 mine.local\n";
        let r = apply_block(orig, &doms()).unwrap();
        let back = remove_block(&r).unwrap();
        assert!(back.contains("1.1.1.1 mine.local"));
    }

    #[test]
    fn domains_parse_skips_comments() {
        let d = parse_domains("# hi\n\nfoo.com\n  bar.com  \n");
        assert_eq!(d, vec!["foo.com", "bar.com"]);
    }
}
