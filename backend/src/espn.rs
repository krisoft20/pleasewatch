use crate::live::LiveGame;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

const TTL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Serialize)]
pub struct EspnTeam {
    pub name: String,
    pub abbr: String,
    pub logo: Option<String>,
    pub color: Option<String>,
    pub record: Option<String>,
    pub score: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EspnMatch {
    pub away: EspnTeam,
    pub home: EspnTeam,
    pub detail: String,
    pub state: String,
    pub network: Option<String>,
}

#[derive(Debug, Clone)]
struct Event {
    away: EspnTeam,
    home: EspnTeam,
    away_keys: Vec<String>,
    home_keys: Vec<String>,
    detail: String,
    state: String,
    network: Option<String>,
}

fn espn_path(sport: &str) -> Option<&'static str> {
    Some(match sport {
        "cfb" => "football/college-football/scoreboard?groups=90&limit=300",
        "nfl" => "football/nfl/scoreboard",
        "mlb" => "baseball/mlb/scoreboard",
        "nba" => "basketball/nba/scoreboard",
        "wnba" => "basketball/wnba/scoreboard",
        "ncaab" => "basketball/mens-college-basketball/scoreboard?groups=50&limit=300",
        "nhl" => "hockey/nhl/scoreboard",
        "premier-league" => "soccer/eng.1/scoreboard",
        "championship" => "soccer/eng.2/scoreboard",
        "womens-super-league" => "soccer/eng.w.1/scoreboard",
        "bundesliga" => "soccer/ger.1/scoreboard",
        "laliga" => "soccer/esp.1/scoreboard",
        "serie-a" => "soccer/ita.1/scoreboard",
        "ligue-1" => "soccer/fra.1/scoreboard",
        "mls" => "soccer/usa.1/scoreboard",
        _ => return None,
    })
}

struct Cached {
    at: Instant,
    events: Vec<Event>,
}
static BOARDS: LazyLock<RwLock<HashMap<String, Cached>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub async fn enrich(games: &mut [LiveGame]) {
    let mut want: Vec<String> = Vec::new();
    for g in games.iter() {
        if g.away.is_empty() || g.home.is_empty() {
            continue;
        }
        if espn_path(&g.sport).is_some() && !want.contains(&g.sport) {
            want.push(g.sport.clone());
        }
    }
    if want.is_empty() {
        return;
    }

    let mut set = tokio::task::JoinSet::new();
    for sport in want {
        set.spawn(async move { (sport.clone(), board(&sport).await) });
    }
    let mut boards: HashMap<String, Vec<Event>> = HashMap::new();
    while let Some(Ok((sport, events))) = set.join_next().await {
        boards.insert(sport, events);
    }

    let mut hits = 0;
    for g in games.iter_mut() {
        if g.away.is_empty() || g.home.is_empty() {
            continue;
        }
        let Some(events) = boards.get(&g.sport) else {
            continue;
        };
        if let Some(m) = best_match(&g.away, &g.home, events) {
            g.espn = Some(m);
            hits += 1;
        }
    }
    eprintln!("[espn] enriched {hits}/{} games", games.len());
}

async fn board(sport: &str) -> Vec<Event> {
    if let Some(c) = BOARDS.read().await.get(sport) {
        if c.at.elapsed() < TTL {
            return c.events.clone();
        }
    }
    let Some(path) = espn_path(sport) else {
        return Vec::new();
    };
    let url = if path.starts_with("soccer/") {
        let today = chrono::Utc::now().date_naive();
        let sep = if path.contains('?') { '&' } else { '?' };
        format!(
            "https://site.api.espn.com/apis/site/v2/sports/{path}{sep}dates={}-{}",
            today.format("%Y%m%d"),
            (today + chrono::Duration::days(8)).format("%Y%m%d")
        )
    } else {
        format!("https://site.api.espn.com/apis/site/v2/sports/{path}")
    };
    let events = match fetch(&url).await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[espn] {sport} board: {e}");
            return BOARDS
                .read()
                .await
                .get(sport)
                .map(|c| c.events.clone())
                .unwrap_or_default();
        }
    };
    BOARDS.write().await.insert(
        sport.to_string(),
        Cached {
            at: Instant::now(),
            events: events.clone(),
        },
    );
    events
}

const UA: &str = "pleasewatch/0.1 (+https://github.com/krisoft20/pleasewatch)";

async fn fetch(url: &str) -> Result<Vec<Event>, String> {
    let text = reqwest::Client::builder()
        .user_agent(UA)
        .http1_only()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    let body: Value = serde_json::from_str(&text).map_err(|e| {
        format!("{e} (got {} bytes starting {:?})", text.len(), &text[..text.len().min(60)])
    })?;
    Ok(parse_board(&body))
}

fn parse_board(body: &Value) -> Vec<Event> {
    let mut out = Vec::new();
    for ev in body["events"].as_array().unwrap_or(&Vec::new()) {
        let Some(comp) = ev["competitions"].get(0) else {
            continue;
        };
        let ty = &comp["status"]["type"];
        let detail = ty["shortDetail"].as_str().unwrap_or("").to_string();
        let state = ty["state"].as_str().unwrap_or("").to_string();
        let network = comp["broadcasts"]
            .get(0)
            .and_then(|b| b["names"].get(0))
            .and_then(Value::as_str)
            .map(str::to_string);

        let mut away: Option<(EspnTeam, Vec<String>)> = None;
        let mut home: Option<(EspnTeam, Vec<String>)> = None;
        for c in comp["competitors"].as_array().unwrap_or(&Vec::new()) {
            let side = c["homeAway"].as_str().unwrap_or("");
            let t = &c["team"];
            let team = EspnTeam {
                name: str_of(&t["shortDisplayName"])
                    .or_else(|| str_of(&t["displayName"]))
                    .unwrap_or_default(),
                abbr: str_of(&t["abbreviation"]).unwrap_or_default(),
                logo: str_of(&t["logo"]),
                color: str_of(&t["color"]).map(|c| format!("#{c}")),
                record: c["records"]
                    .get(0)
                    .and_then(|r| str_of(&r["summary"])),
                score: str_of(&c["score"]),
            };
            let keys = [
                str_of(&t["displayName"]),
                str_of(&t["shortDisplayName"]),
                str_of(&t["location"]),
                str_of(&t["abbreviation"]),
            ]
            .into_iter()
            .flatten()
            .map(|s| norm(&s))
            .filter(|s| !s.is_empty())
            .collect();
            match side {
                "away" => away = Some((team, keys)),
                "home" => home = Some((team, keys)),
                _ => {}
            }
        }
        if let (Some((a, ak)), Some((h, hk))) = (away, home) {
            out.push(Event {
                away: a,
                home: h,
                away_keys: ak,
                home_keys: hk,
                detail,
                state,
                network,
            });
        }
    }
    out
}

fn str_of(v: &Value) -> Option<String> {
    v.as_str().map(str::to_string).filter(|s| !s.is_empty())
}

fn norm(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '&')
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn score_name(q: &str, keys: &[String]) -> u32 {
    if q.is_empty() {
        return 0;
    }
    let mut best = 0;
    for k in keys {
        let s = if k == q {
            4
        } else if k.starts_with(q) || q.starts_with(k.as_str()) {
            if q.len() >= 4 && k.len() >= 4 {
                3
            } else {
                0
            }
        } else if q.len() >= 5 && k.contains(q) {
            2
        } else if k.len() >= 5 && q.contains(k.as_str()) {
            2
        } else {
            0
        };
        best = best.max(s);
    }
    best
}

fn best_match(away: &str, home: &str, events: &[Event]) -> Option<EspnMatch> {
    let (qa, qh) = (norm(away), norm(home));
    let mut best: Option<(u32, &Event, bool)> = None;
    for e in events {
        let straight = {
            let a = score_name(&qa, &e.away_keys);
            let b = score_name(&qh, &e.home_keys);
            if a >= 2 && b >= 2 {
                a + b
            } else {
                0
            }
        };
        let swapped = {
            let a = score_name(&qa, &e.home_keys);
            let b = score_name(&qh, &e.away_keys);
            if a >= 2 && b >= 2 {
                a + b
            } else {
                0
            }
        };
        let (total, flip) = if swapped > straight {
            (swapped, true)
        } else {
            (straight, false)
        };
        if total > 0 && best.map(|(t, _, _)| total > t).unwrap_or(true) {
            best = Some((total, e, flip));
        }
    }
    let (_, e, flip) = best?;
    let (a, h) = if flip {
        (e.home.clone(), e.away.clone())
    } else {
        (e.away.clone(), e.home.clone())
    };
    Some(EspnMatch {
        away: a,
        home: h,
        detail: e.detail.clone(),
        state: e.state.clone(),
        network: e.network.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(a: &str, h: &str) -> Event {
        let t = |n: &str| EspnTeam {
            name: n.into(),
            abbr: n.into(),
            logo: None,
            color: None,
            record: None,
            score: Some("0".into()),
        };
        Event {
            away: t(a),
            home: t(h),
            away_keys: vec![norm(a)],
            home_keys: vec![norm(h)],
            detail: "d".into(),
            state: "in".into(),
            network: None,
        }
    }

    #[test]
    fn exact_beats_containment() {
        let events = vec![ev("Western Michigan", "Michigan")];
        let m = best_match("Michigan", "Western Michigan", &events).unwrap();
        assert_eq!(m.away.name, "Michigan");
        assert_eq!(m.home.name, "Western Michigan");
    }

    #[test]
    fn orientation_flips_when_the_index_reversed_it() {
        let events = vec![ev("Missouri State", "Texas A&M")];
        let m = best_match("Texas A&M", "Missouri State", &events).unwrap();
        assert_eq!(m.away.name, "Texas A&M");
        assert_eq!(m.home.name, "Missouri State");
    }

    #[test]
    fn one_sided_match_is_rejected() {
        let events = vec![ev("Alabama", "Georgia")];
        assert!(best_match("Alabama", "Some Other School", &events).is_none());
    }

    #[test]
    fn trailing_mascot_still_matches() {
        let mut e = ev("Montana State", "Butler");
        e.away_keys = vec![norm("Montana State Bobcats"), norm("Montana State")];
        e.home_keys = vec![norm("Butler Bulldogs"), norm("Butler")];
        let m = best_match("Montana State Bobcats", "Butler Bulldogs", &[e]).unwrap();
        assert_eq!(m.detail, "d");
    }

    #[test]
    fn parses_a_scoreboard_payload() {
        let body: Value = serde_json::from_str(
            r#"{"events":[{"competitions":[{
              "status":{"type":{"state":"in","shortDetail":"2:09 - 3rd"}},
              "broadcasts":[{"names":["ESPN"]}],
              "competitors":[
                {"homeAway":"home","score":"26","records":[{"summary":"1-0"}],
                 "team":{"displayName":"Texas A&M Aggies","shortDisplayName":"Texas A&M","location":"Texas A&M","abbreviation":"TA&M","color":"500000","logo":"http://l/1.png"}},
                {"homeAway":"away","score":"0","records":[{"summary":"0-1"}],
                 "team":{"displayName":"Missouri State Bears","shortDisplayName":"Missouri St","location":"Missouri State","abbreviation":"MOST","color":"5e0009","logo":"http://l/2.png"}}
              ]}]}]}"#,
        )
        .unwrap();
        let events = parse_board(&body);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].home.score.as_deref(), Some("26"));
        assert_eq!(events[0].home.color.as_deref(), Some("#500000"));
        assert_eq!(events[0].network.as_deref(), Some("ESPN"));
        let m = best_match("Missouri State", "Texas A&M", &events).unwrap();
        assert_eq!(m.away.abbr, "MOST");
        assert_eq!(m.home.abbr, "TA&M");
        assert_eq!(m.detail, "2:09 - 3rd");
    }
}
