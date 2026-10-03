use crate::{middleware::AuthUser, AppState};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};
use tokio::sync::Mutex;

const DIM: usize = 300;
const THRESHOLD: f32 = 0.32;
const ROWS: [(&str, usize); 3] = [("en", 100_000), ("pl", 200_000), ("de", 200_000)];

struct Vectors {
    index: HashMap<String, u32>,
    scale: Vec<f32>,
    data: Vec<i8>,
}

impl Vectors {
    fn get(&self, w: &str) -> Option<(f32, &[i8])> {
        let i = *self.index.get(w)? as usize;
        Some((self.scale[i], &self.data[i * DIM..(i + 1) * DIM]))
    }
}

static LOADED: LazyLock<Mutex<HashMap<&'static str, Arc<Vectors>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn lang_key(code: &str) -> Option<&'static str> {
    match code.to_lowercase().as_str() {
        "en" | "eng" | "english" => Some("en"),
        "pl" | "pol" | "polish" => Some("pl"),
        "de" | "ger" | "deu" | "german" => Some("de"),
        _ => None,
    }
}

fn dir() -> PathBuf {
    let db = std::env::var("DATABASE_PATH").unwrap_or("data/pleasewatch.db".into());
    PathBuf::from(db)
        .parent()
        .map(|p| p.join("align"))
        .unwrap_or_else(|| "align".into())
}

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/align", post(align_handler))
        .layer(axum::middleware::from_fn_with_state(
            state,
            crate::middleware::require_auth,
        ))
}

async fn vectors(lang: &'static str) -> Result<Arc<Vectors>, String> {
    let mut loaded = LOADED.lock().await;
    if let Some(v) = loaded.get(lang) {
        return Ok(v.clone());
    }
    let rows = ROWS
        .iter()
        .find(|(l, _)| *l == lang)
        .map(|(_, n)| *n)
        .unwrap_or(100_000);
    let bin = dir().join(format!("{lang}.bin"));
    let v = if bin.exists() {
        tokio::task::spawn_blocking(move || read_bin(&bin))
            .await
            .map_err(|e| e.to_string())??
    } else {
        let v = fetch(lang, rows).await?;
        let (path, v) = tokio::task::spawn_blocking(move || {
            let r = write_bin(&bin, &v);
            (r.map(|_| bin), v)
        })
        .await
        .map_err(|e| e.to_string())?;
        if let Err(e) = path {
            crate::pe!("[align] cache {lang}: {e}");
        }
        v
    };
    crate::pi!("[align] {lang}: {} words loaded", v.scale.len());
    let v = Arc::new(v);
    loaded.insert(lang, v.clone());
    Ok(v)
}

async fn fetch(lang: &str, rows: usize) -> Result<Vectors, String> {
    let url =
        format!("https://dl.fbaipublicfiles.com/fasttext/vectors-aligned/wiki.{lang}.align.vec");
    crate::pi!("[align] downloading top {rows} {lang} vectors");
    let mut resp = reqwest::get(&url).await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("vectors http {}", resp.status()));
    }
    let mut v = Vectors {
        index: HashMap::with_capacity(rows),
        scale: Vec::with_capacity(rows),
        data: Vec::with_capacity(rows * DIM),
    };
    let mut buf: Vec<u8> = Vec::new();
    let mut header = true;
    while v.scale.len() < rows {
        let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? else {
            break;
        };
        buf.extend_from_slice(&chunk);
        let mut start = 0;
        while let Some(nl) = buf[start..].iter().position(|&b| b == b'\n') {
            let line = String::from_utf8_lossy(&buf[start..start + nl]).into_owned();
            start += nl + 1;
            if std::mem::take(&mut header) {
                continue;
            }
            push_line(&mut v, &line);
            if v.scale.len() >= rows {
                break;
            }
        }
        buf.drain(..start);
    }
    Ok(v)
}

fn push_line(v: &mut Vectors, line: &str) {
    let mut parts = line.split(' ');
    let Some(word) = parts.next() else { return };
    let xs: Vec<f32> = parts.filter_map(|p| p.trim().parse().ok()).collect();
    if xs.len() != DIM || v.index.contains_key(word) {
        return;
    }
    let norm = xs.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-6);
    let max = xs.iter().fold(0f32, |m, x| m.max(x.abs())) / norm;
    let scale = max.max(1e-6) / 127.0;
    v.index.insert(word.to_string(), v.scale.len() as u32);
    v.scale.push(scale);
    v.data.extend(
        xs.iter()
            .map(|x| (x / norm / scale).round().clamp(-127.0, 127.0) as i8),
    );
}

fn write_bin(path: &std::path::Path, v: &Vectors) -> std::io::Result<()> {
    std::fs::create_dir_all(path.parent().unwrap())?;
    let mut words = vec![""; v.scale.len()];
    for (w, &i) in &v.index {
        words[i as usize] = w;
    }
    let tmp = path.with_extension("tmp");
    let mut f = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
    f.write_all(&(words.len() as u32).to_le_bytes())?;
    for (i, w) in words.iter().enumerate() {
        let b = &w.as_bytes()[..w.len().min(255)];
        f.write_all(&[b.len() as u8])?;
        f.write_all(b)?;
        f.write_all(&v.scale[i].to_le_bytes())?;
        let row: Vec<u8> = v.data[i * DIM..(i + 1) * DIM]
            .iter()
            .map(|&x| x as u8)
            .collect();
        f.write_all(&row)?;
    }
    f.flush()?;
    drop(f);
    std::fs::rename(tmp, path)
}

fn read_bin(path: &std::path::Path) -> Result<Vectors, String> {
    let mut f = std::io::BufReader::new(std::fs::File::open(path).map_err(|e| e.to_string())?);
    let mut n = [0u8; 4];
    f.read_exact(&mut n).map_err(|e| e.to_string())?;
    let n = u32::from_le_bytes(n) as usize;
    let mut v = Vectors {
        index: HashMap::with_capacity(n),
        scale: Vec::with_capacity(n),
        data: vec![0; n * DIM],
    };
    for i in 0..n {
        let mut len = [0u8; 1];
        f.read_exact(&mut len).map_err(|e| e.to_string())?;
        let mut w = vec![0u8; len[0] as usize];
        f.read_exact(&mut w).map_err(|e| e.to_string())?;
        let mut s = [0u8; 4];
        f.read_exact(&mut s).map_err(|e| e.to_string())?;
        let mut row = vec![0u8; DIM];
        f.read_exact(&mut row).map_err(|e| e.to_string())?;
        v.index
            .insert(String::from_utf8_lossy(&w).into_owned(), i as u32);
        v.scale.push(f32::from_le_bytes(s));
        for (d, b) in v.data[i * DIM..(i + 1) * DIM].iter_mut().zip(row) {
            *d = b as i8;
        }
    }
    Ok(v)
}

pub fn words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = text.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        let joiner =
            c == '\'' && !cur.is_empty() && chars.get(i + 1).is_some_and(|n| n.is_alphabetic());
        if c.is_alphabetic() || joiner {
            cur.extend(c.to_lowercase());
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn stop(lang: &str) -> &'static [&'static str] {
    match lang {
        "en" => &[
            "a", "an", "the", "i", "me", "my", "you", "your", "he", "him", "his", "she", "her",
            "it", "its", "we", "us", "our", "they", "them", "their", "this", "that", "these",
            "those", "is", "am", "are", "was", "were", "be", "been", "being", "have", "has", "had",
            "do", "does", "did", "doing", "will", "would", "can", "could", "should", "shall",
            "may", "might", "must", "to", "of", "in", "on", "at", "by", "for", "with", "from",
            "up", "out", "about", "into", "over", "and", "or", "but", "so", "if", "then", "than",
            "as", "not", "no", "yes", "oh", "hey", "well", "just", "don't", "i'm", "you're",
            "it's", "that's", "let's", "we're", "he's", "she's", "what's", "there's",
        ],
        "pl" => &[
            "a",
            "i",
            "o",
            "u",
            "w",
            "z",
            "na",
            "do",
            "od",
            "po",
            "za",
            "ze",
            "we",
            "ku",
            "to",
            "ten",
            "ta",
            "te",
            "tego",
            "tej",
            "tym",
            "tę",
            "ja",
            "ty",
            "on",
            "ona",
            "ono",
            "my",
            "wy",
            "oni",
            "one",
            "mnie",
            "mi",
            "mną",
            "cię",
            "ci",
            "ciebie",
            "tobą",
            "go",
            "mu",
            "jego",
            "jej",
            "ją",
            "je",
            "ich",
            "im",
            "nas",
            "nam",
            "nami",
            "was",
            "wam",
            "wami",
            "się",
            "sobie",
            "siebie",
            "jest",
            "są",
            "być",
            "był",
            "była",
            "było",
            "byli",
            "jestem",
            "jesteś",
            "jesteśmy",
            "że",
            "czy",
            "nie",
            "tak",
            "ale",
            "bo",
            "co",
            "jak",
            "już",
            "też",
            "tylko",
            "no",
            "oraz",
            "albo",
            "lub",
            "więc",
            "gdy",
            "kiedy",
            "tu",
            "tam",
        ],
        "de" => &[
            "der", "die", "das", "den", "dem", "des", "ein", "eine", "einen", "einem", "einer",
            "eines", "ich", "du", "er", "sie", "es", "wir", "ihr", "mich", "mir", "dich", "dir",
            "ihn", "ihm", "uns", "euch", "sich", "mein", "dein", "sein", "und", "oder", "aber",
            "doch", "denn", "so", "wie", "als", "wenn", "ob", "dass", "zu", "in", "im", "an", "am",
            "auf", "aus", "bei", "mit", "nach", "von", "vor", "für", "über", "um", "ist", "bin",
            "bist", "sind", "war", "waren", "hat", "habe", "haben", "hast", "nicht", "kein",
            "keine", "ja", "nein", "nur", "auch", "schon", "noch", "was", "wer",
        ],
        _ => &[],
    }
}

const FUNC: &[(&[&str], &[&str], &[&str])] = &[
    (&["yes", "yeah", "yep"], &["tak"], &["ja"]),
    (&["no", "nope"], &["nie"], &["nein"]),
    (
        &[
            "not", "don't", "doesn't", "didn't", "isn't", "aren't", "wasn't", "won't", "can't",
            "never",
        ],
        &["nie", "nigdy"],
        &[
            "nicht", "kein", "keine", "keinen", "keinem", "keiner", "nie", "niemals",
        ],
    ),
    (&["and"], &["i", "oraz"], &["und"]),
    (&["but"], &["ale", "lecz"], &["aber", "sondern"]),
    (&["or"], &["albo", "lub", "czy"], &["oder"]),
    (
        &["i", "me"],
        &["ja", "mnie", "mi", "mną"],
        &["ich", "mich", "mir"],
    ),
    (
        &["you"],
        &[
            "ty", "cię", "ciebie", "ci", "tobą", "tobie", "wy", "was", "wam", "wami",
        ],
        &["du", "dich", "dir", "ihr", "euch"],
    ),
    (
        &["he", "him"],
        &["on", "go", "jego", "mu", "niego", "niemu", "nim"],
        &["er", "ihn", "ihm"],
    ),
    (
        &["she", "her"],
        &["ona", "ją", "jej", "nią", "niej"],
        &["sie", "ihr"],
    ),
    (
        &["we", "us"],
        &["my", "nas", "nam", "nami"],
        &["wir", "uns"],
    ),
    (
        &["they", "them"],
        &["oni", "one", "ich", "im", "nich", "nimi"],
        &["sie", "ihnen"],
    ),
    (
        &["my", "mine"],
        &[
            "mój", "moja", "moje", "mojego", "mojej", "moim", "moją", "moi", "moich", "moimi",
        ],
        &["mein", "meine", "meinen", "meinem", "meiner", "meines"],
    ),
    (
        &["your", "yours"],
        &[
            "twój", "twoja", "twoje", "twojego", "twojej", "twoim", "twoją", "twoi", "twoich",
            "wasz", "wasza", "wasze",
        ],
        &[
            "dein", "deine", "deinen", "deinem", "deiner", "deines", "euer", "eure", "euren",
        ],
    ),
    (&["what"], &["co", "czego", "czym", "czemu"], &["was"]),
    (
        &["who", "whom"],
        &["kto", "kogo", "komu", "kim"],
        &["wer", "wen", "wem"],
    ),
    (&["where"], &["gdzie", "dokąd"], &["wo", "wohin"]),
    (&["when"], &["kiedy", "gdy"], &["wann", "wenn", "als"]),
    (&["why"], &["dlaczego", "czemu"], &["warum", "wieso"]),
    (&["how"], &["jak"], &["wie"]),
    (&["with"], &["z", "ze"], &["mit"]),
    (&["without"], &["bez"], &["ohne"]),
    (&["for"], &["dla"], &["für"]),
    (&["from"], &["od", "z", "ze"], &["von", "aus"]),
    (&["after"], &["po"], &["nach"]),
    (&["before"], &["przed"], &["vor"]),
    (&["in", "inside"], &["w", "we"], &["in", "im"]),
    (&["on"], &["na"], &["auf", "an", "am"]),
    (&["here"], &["tu", "tutaj"], &["hier"]),
    (&["there"], &["tam"], &["da", "dort"]),
    (&["also", "too"], &["też", "także", "również"], &["auch"]),
    (&["already"], &["już"], &["schon"]),
    (&["only", "just"], &["tylko"], &["nur"]),
    (&["still"], &["jeszcze", "nadal", "wciąż"], &["noch"]),
    (&["that"], &["że"], &["dass"]),
    (
        &["if"],
        &["jeśli", "jeżeli", "gdyby"],
        &["wenn", "falls", "ob"],
    ),
    (
        &["because", "cause"],
        &["bo", "ponieważ"],
        &["weil", "denn"],
    ),
    (
        &["is", "am", "are"],
        &["jest", "jestem", "jesteś", "są", "jesteśmy", "jesteście"],
        &["ist", "bin", "bist", "sind", "seid"],
    ),
    (
        &["was", "were"],
        &[
            "był", "była", "było", "byli", "były", "byłem", "byłam", "byłeś", "byłaś",
        ],
        &["war", "waren", "warst", "wart"],
    ),
    (
        &["have", "has", "had"],
        &[
            "mam", "masz", "ma", "mamy", "macie", "mają", "miał", "miała", "mieli",
        ],
        &["habe", "hast", "hat", "haben", "habt", "hatte", "hatten"],
    ),
];

fn func_forms(
    row: &(
        &'static [&'static str],
        &'static [&'static str],
        &'static [&'static str],
    ),
    lang: &str,
) -> &'static [&'static str] {
    match lang {
        "en" => row.0,
        "pl" => row.1,
        _ => row.2,
    }
}

fn pairs(
    va: &Vectors,
    la: &str,
    a: &str,
    vb: &Vectors,
    lb: &str,
    b: &str,
) -> Vec<(String, String)> {
    let mut out = vector_pairs(va, la, a, vb, lb, b);
    let taken_a: std::collections::HashSet<String> = out.iter().map(|p| p.0.clone()).collect();
    let mut taken_b: std::collections::HashSet<String> = out.iter().map(|p| p.1.clone()).collect();
    let wb = words(b);
    let mut seen = std::collections::HashSet::new();
    for w in words(a) {
        if taken_a.contains(&w) || !seen.insert(w.clone()) {
            continue;
        }
        let hit = FUNC
            .iter()
            .filter(|row| func_forms(row, la).contains(&w.as_str()))
            .find_map(|row| {
                let forms = func_forms(row, lb);
                wb.iter()
                    .find(|x| !taken_b.contains(*x) && forms.contains(&x.as_str()))
                    .cloned()
            });
        if let Some(x) = hit {
            taken_b.insert(x.clone());
            out.push((w, x));
        }
    }
    out
}

fn vector_pairs(
    va: &Vectors,
    la: &str,
    a: &str,
    vb: &Vectors,
    lb: &str,
    b: &str,
) -> Vec<(String, String)> {
    let keep = |ws: Vec<String>, lang: &str, v: &Vectors| -> Vec<(String, f32, Vec<i8>)> {
        let stop = stop(lang);
        let mut seen = std::collections::HashSet::new();
        ws.into_iter()
            .filter(|w| !stop.contains(&w.as_str()) && seen.insert(w.clone()))
            .filter_map(|w| v.get(&w).map(|(s, d)| (w, s, d.to_vec())))
            .collect()
    };
    let wa = keep(words(a), la, va);
    let wb = keep(words(b), lb, vb);
    if wa.is_empty() || wb.is_empty() {
        return Vec::new();
    }
    let sim: Vec<Vec<f32>> = wa
        .iter()
        .map(|(_, sa, da)| {
            wb.iter()
                .map(|(_, sb, db)| {
                    sa * sb
                        * da.iter()
                            .zip(db)
                            .map(|(&x, &y)| x as i32 * y as i32)
                            .sum::<i32>() as f32
                })
                .collect()
        })
        .collect();
    let mut out = Vec::new();
    for (i, row) in sim.iter().enumerate() {
        let (j, &best) = row
            .iter()
            .enumerate()
            .max_by(|x, y| x.1.total_cmp(y.1))
            .unwrap();
        if best < THRESHOLD {
            continue;
        }
        let back = (0..wa.len())
            .max_by(|&x, &y| sim[x][j].total_cmp(&sim[y][j]))
            .unwrap();
        if back == i {
            out.push((wa[i].0.clone(), wb[j].0.clone()));
        }
    }
    out
}

#[derive(Deserialize)]
struct AlignReq {
    a_lang: String,
    b_lang: String,
    a: String,
    b: String,
}

#[derive(Serialize)]
struct AlignResp {
    pairs: Vec<(String, String)>,
}

async fn align_handler(
    Extension(_auth): Extension<AuthUser>,
    Json(req): Json<AlignReq>,
) -> Response {
    let (Some(la), Some(lb)) = (lang_key(&req.a_lang), lang_key(&req.b_lang)) else {
        return (StatusCode::UNPROCESSABLE_ENTITY, "language not supported").into_response();
    };
    if req.a.len() > 2000 || req.b.len() > 2000 {
        return (StatusCode::PAYLOAD_TOO_LARGE, "line too long").into_response();
    }
    let (va, vb) = match (vectors(la).await, vectors(lb).await) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(e), _) | (_, Err(e)) => {
            crate::pe!("[align] vectors: {e}");
            return (StatusCode::SERVICE_UNAVAILABLE, "vectors unavailable").into_response();
        }
    };
    Json(AlignResp {
        pairs: pairs(&va, la, &req.a, &vb, lb, &req.b),
    })
    .into_response()
}

pub fn spawn_warmup() {
    tokio::spawn(async {
        for (lang, _) in ROWS {
            if let Err(e) = vectors(lang).await {
                crate::pe!("[align] warmup {lang}: {e}");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn function_words_pair_through_the_table() {
        let none = Vectors {
            index: HashMap::new(),
            scale: Vec::new(),
            data: Vec::new(),
        };
        let p = pairs(
            &none,
            "pl",
            "Nie, nie. Ja tego nie wiem, ale ty tak.",
            &none,
            "de",
            "Nein, nein. Ich weiß das nicht, aber du ja.",
        );
        for want in [
            ("nie", "nein"),
            ("ja", "ich"),
            ("ale", "aber"),
            ("ty", "du"),
            ("tak", "ja"),
        ] {
            assert!(
                p.contains(&(want.0.into(), want.1.into())),
                "missing {want:?} in {p:?}"
            );
        }
        let p = pairs(
            &none,
            "en",
            "Yes, I don't know.",
            &none,
            "pl",
            "Tak, nie wiem.",
        );
        assert!(
            p.contains(&("yes".into(), "tak".into()))
                && p.contains(&("don't".into(), "nie".into())),
            "{p:?}"
        );
    }

    #[test]
    fn words_keeps_contractions_and_polish_letters() {
        assert_eq!(
            words("Don't stop, Łódź! It's 5pm"),
            vec!["don't", "stop", "łódź", "it's", "pm"]
        );
        assert_eq!(words("'quoted' rock'n'roll"), vec!["quoted", "rock'n'roll"]);
    }

    #[test]
    #[ignore]
    fn live_pairs() {
        let d = PathBuf::from(std::env::var("ALIGN_TEST_DIR").expect("ALIGN_TEST_DIR"));
        let en = read_bin(&d.join("en.bin")).unwrap();
        let pl = read_bin(&d.join("pl.bin")).unwrap();
        let de = read_bin(&d.join("de.bin")).unwrap();
        let cases = [
            (
                "en",
                &en,
                "Allie, his office doesn't open for an hour.",
                "pl",
                &pl,
                "Allie, on otwiera dopiero za godzinę.",
            ),
            (
                "en",
                &en,
                "Yeah. Well, we can get some breakfast.",
                "pl",
                &pl,
                "Tak. Możemy zjeść śniadanie.",
            ),
            (
                "pl",
                &pl,
                "Nie wiem, po co to robimy. Próbowaliśmy już dwa razy.",
                "de",
                &de,
                "Wozu machen wir das? Wir hatten zwei Versuche.",
            ),
            (
                "en",
                &en,
                "Hey, you could be pregnant a month from now.",
                "de",
                &de,
                "In einem Monat könntest du schwanger sein.",
            ),
        ];
        for (la, va, a, lb, vb, b) in cases {
            println!("{a}\n{b}\n  -> {:?}", pairs(va, la, a, vb, lb, b));
        }
        let p = pairs(&en, "en", cases[0].2, &pl, "pl", cases[0].5);
        assert!(p.contains(&("hour".into(), "godzinę".into())));
    }

    #[test]
    fn quantized_roundtrip_through_bin() {
        let mut v = Vectors {
            index: HashMap::new(),
            scale: Vec::new(),
            data: Vec::new(),
        };
        let row: Vec<String> = (0..DIM)
            .map(|i| format!("{:.4}", (i as f32 * 0.37).sin() * 0.1))
            .collect();
        push_line(&mut v, &format!("hour {}", row.join(" ")));
        push_line(&mut v, &format!("godzinę {}", row.join(" ")));
        let p = std::env::temp_dir().join(format!("pw-align-{}.bin", std::process::id()));
        write_bin(&p, &v).unwrap();
        let r = read_bin(&p).unwrap();
        let _ = std::fs::remove_file(&p);
        let (s1, d1) = r.get("hour").unwrap();
        let (s2, d2) = r.get("godzinę").unwrap();
        let cos = s1
            * s2
            * d1.iter()
                .zip(d2)
                .map(|(&x, &y)| x as i32 * y as i32)
                .sum::<i32>() as f32;
        assert!((cos - 1.0).abs() < 0.02, "cos {cos}");
    }
}
