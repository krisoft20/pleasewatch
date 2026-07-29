pub fn lang_name(code: &str) -> String {
    let lc = code.to_lowercase();
    let short = lc.split(['-', '_']).next().unwrap_or(lc.as_str());
    let mapped: &str = match short {
        "en" | "eng" | "english" => "English",
        "pl" | "pol" | "polish" => "Polish",
        "de" | "ger" | "deu" | "german" => "German",
        "es" | "spa" | "spanish" => "Spanish",
        "fr" | "fre" | "fra" | "french" => "French",
        "it" | "ita" | "italian" => "Italian",
        "pt" | "por" | "portuguese" => "Portuguese",
        "ru" | "rus" | "russian" => "Russian",
        "ja" | "jpn" | "japanese" => "Japanese",
        "ko" | "kor" | "korean" => "Korean",
        "zh" | "chi" | "zho" | "chinese" => "Chinese",
        "ar" | "ara" | "arabic" => "Arabic",
        "tr" | "tur" | "turkish" => "Turkish",
        "nl" | "dut" | "nld" | "dutch" => "Dutch",
        "sv" | "swe" | "swedish" => "Swedish",
        "no" | "nor" | "norwegian" => "Norwegian",
        "da" | "dan" | "danish" => "Danish",
        "fi" | "fin" | "finnish" => "Finnish",
        "cs" | "cze" | "ces" | "czech" => "Czech",
        "sk" | "slk" | "slo" | "slovak" => "Slovak",
        "hu" | "hun" | "hungarian" => "Hungarian",
        "ro" | "rum" | "ron" | "romanian" => "Romanian",
        "el" | "gre" | "ell" | "greek" => "Greek",
        "he" | "heb" | "hebrew" => "Hebrew",
        "hi" | "hin" | "hindi" => "Hindi",
        "id" | "ind" | "indonesian" => "Indonesian",
        "ms" | "may" | "msa" | "malay" => "Malay",
        "th" | "tha" | "thai" => "Thai",
        "vi" | "vie" | "vietnamese" => "Vietnamese",
        "uk" | "ukr" | "ukrainian" => "Ukrainian",
        "bg" | "bul" | "bulgarian" => "Bulgarian",
        "sr" | "srp" | "serbian" => "Serbian",
        "hr" | "hrv" | "croatian" => "Croatian",
        "sl" | "slv" | "slovenian" => "Slovenian",
        "bs" | "bos" | "bosnian" => "Bosnian",
        "mk" | "mac" | "mkd" | "macedonian" => "Macedonian",
        "et" | "est" | "estonian" => "Estonian",
        "lv" | "lav" | "latvian" => "Latvian",
        "lt" | "lit" | "lithuanian" => "Lithuanian",
        "ca" | "cat" | "catalan" => "Catalan",
        "ga" | "gle" | "irish" => "Irish",
        "is" | "ice" | "isl" | "icelandic" => "Icelandic",
        "fa" | "per" | "fas" | "persian" => "Persian",
        "ur" | "urd" | "urdu" => "Urdu",
        "bn" | "ben" | "bengali" => "Bengali",
        "ta" | "tam" | "tamil" => "Tamil",
        "te" | "tel" | "telugu" => "Telugu",
        "ml" | "mal" | "malayalam" => "Malayalam",
        "mr" | "mar" | "marathi" => "Marathi",
        "gu" | "guj" | "gujarati" => "Gujarati",
        "pa" | "pan" | "punjabi" => "Punjabi",
        "sw" | "swa" | "swahili" => "Swahili",
        "af" | "afr" | "afrikaans" => "Afrikaans",
        "tl" | "tgl" | "filipino" | "tagalog" => "Filipino",
        "az" | "aze" | "azerbaijani" => "Azerbaijani",
        "kk" | "kaz" | "kazakh" => "Kazakh",
        "ka" | "geo" | "kat" | "georgian" => "Georgian",
        "hy" | "arm" | "hye" | "armenian" => "Armenian",
        "be" | "bel" | "belarusian" => "Belarusian",
        "und" | "" => "Unknown",
        _ => return code.to_string(),
    };
    mapped.to_string()
}

pub fn canon_lang_code(code: &str) -> Option<&'static str> {
    let lc = code.trim().to_lowercase();
    let short = lc.split(['-', '_']).next().unwrap_or(lc.as_str());
    match short {
        "en" | "eng" | "english" => Some("eng"),
        "pl" | "pol" | "polish" => Some("pol"),
        "de" | "ger" | "deu" | "german" => Some("ger"),
        "es" | "spa" | "spanish" => Some("spa"),
        "fr" | "fre" | "fra" | "french" => Some("fre"),
        "it" | "ita" | "italian" => Some("ita"),
        "pt" | "por" | "portuguese" => Some("por"),
        "ru" | "rus" | "russian" => Some("rus"),
        "ja" | "jpn" | "japanese" => Some("jpn"),
        "ko" | "kor" | "korean" => Some("kor"),
        "zh" | "chi" | "zho" | "chinese" => Some("chi"),
        "ar" | "ara" | "arabic" => Some("ara"),
        "tr" | "tur" | "turkish" => Some("tur"),
        "nl" | "dut" | "nld" | "dutch" => Some("dut"),
        "sv" | "swe" | "swedish" => Some("swe"),
        "no" | "nor" | "norwegian" => Some("nor"),
        "da" | "dan" | "danish" => Some("dan"),
        "fi" | "fin" | "finnish" => Some("fin"),
        "cs" | "cze" | "ces" | "czech" => Some("cze"),
        "sk" | "slk" | "slo" | "slovak" => Some("slo"),
        "hu" | "hun" | "hungarian" => Some("hun"),
        "ro" | "rum" | "ron" | "romanian" => Some("rum"),
        "el" | "gre" | "ell" | "greek" => Some("gre"),
        "he" | "heb" | "hebrew" => Some("heb"),
        "hi" | "hin" | "hindi" => Some("hin"),
        "id" | "ind" | "indonesian" => Some("ind"),
        "th" | "tha" | "thai" => Some("tha"),
        "vi" | "vie" | "vietnamese" => Some("vie"),
        "uk" | "ukr" | "ukrainian" => Some("ukr"),
        "und" | "" | "unknown" => None,
        _ => None,
    }
}

pub fn lang_code_from_label(label: &str) -> Option<&'static str> {
    let mut cleaned = String::with_capacity(label.len() + 2);
    cleaned.push(' ');
    for ch in label.chars() {
        if ch.is_ascii_alphabetic() {
            cleaned.push(ch.to_ascii_lowercase());
        } else {
            cleaned.push(' ');
        }
    }
    cleaned.push(' ');

    for (name, code) in [
        ("english", "eng"),
        ("polish", "pol"),
        ("german", "ger"),
        ("spanish", "spa"),
        ("french", "fre"),
        ("italian", "ita"),
        ("portuguese", "por"),
        ("russian", "rus"),
        ("japanese", "jpn"),
        ("korean", "kor"),
        ("chinese", "chi"),
        ("arabic", "ara"),
        ("turkish", "tur"),
        ("dutch", "dut"),
        ("swedish", "swe"),
        ("norwegian", "nor"),
        ("danish", "dan"),
        ("finnish", "fin"),
        ("czech", "cze"),
        ("slovak", "slo"),
        ("hungarian", "hun"),
        ("romanian", "rum"),
        ("greek", "gre"),
        ("hebrew", "heb"),
        ("hindi", "hin"),
        ("indonesian", "ind"),
        ("thai", "tha"),
        ("vietnamese", "vie"),
        ("ukrainian", "ukr"),
    ] {
        let needle = format!(" {name} ");
        if cleaned.contains(&needle) {
            return Some(code);
        }
    }
    None
}

pub fn looks_like_raw_code(s: &str) -> bool {
    let len = s.len();
    if len < 2 || len > 4 {
        return false;
    }
    s.chars().all(|c| c.is_ascii_alphabetic())
}
