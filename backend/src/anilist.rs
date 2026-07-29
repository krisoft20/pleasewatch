use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const AL_URL: &str = "https://graphql.anilist.co";
const CACHE_TTL: Duration = Duration::from_secs(3600);

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
static CACHE: OnceLock<Mutex<HashMap<i64, (Option<Bundle>, Instant)>>> = OnceLock::new();

fn client() -> &'static reqwest::Client {
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent("pleasewatch/0.1")
            .timeout(Duration::from_secs(12))
            .build()
            .expect("reqwest client")
    })
}

fn cache() -> &'static Mutex<HashMap<i64, (Option<Bundle>, Instant)>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Clone, Debug)]
pub struct Bundle {
    pub relations: Vec<RelatedWork>,
    pub recommendations: Vec<Recommended>,
    pub anime_adaptation: Option<AnimeAdaptation>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct RelatedWork {
    pub anilist_id: i64,
    pub relation: String,
    pub title: String,
    pub cover_url: Option<String>,
    pub kind: String,
    pub format: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Recommended {
    pub anilist_id: i64,
    pub title: String,
    pub cover_url: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct AnimeAdaptation {
    pub anilist_id: i64,
    pub title: String,
    pub cover_url: Option<String>,
    pub format: Option<String>,
}

const QUERY: &str = r#"
query ($id: Int) {
  Media(id: $id, type: MANGA) {
    id
    relations {
      edges {
        relationType
        node {
          id
          type
          format
          title { romaji english }
          coverImage { large medium }
        }
      }
    }
    recommendations(perPage: 8, sort: RATING_DESC) {
      nodes {
        mediaRecommendation {
          id
          type
          title { romaji english }
          coverImage { large medium }
        }
      }
    }
  }
}
"#;

pub async fn fetch(anilist_id: i64) -> reqwest::Result<Option<Bundle>> {
    if let Some(hit) = cache_get(anilist_id) {
        return Ok(hit);
    }

    let body = serde_json::json!({
        "query": QUERY,
        "variables": { "id": anilist_id },
    });

    let r = client().post(AL_URL).json(&body).send().await?;
    if !r.status().is_success() {
        eprintln!("[anilist] http {} for id {anilist_id}", r.status());
        cache_put(anilist_id, None);
        return Ok(None);
    }
    let raw: GqlResp = r.json().await?;
    let Some(media) = raw.data.and_then(|d| d.media) else {
        cache_put(anilist_id, None);
        return Ok(None);
    };

    let mut relations: Vec<RelatedWork> = Vec::new();
    let mut anime_adaptation: Option<AnimeAdaptation> = None;
    for e in media.relations.edges {
        let title = pick_title(&e.node.title);
        let cover = e.node.cover_image.and_then(|c| c.large.or(c.medium));
        let kind = e.node.kind.unwrap_or_default();

        if kind == "ANIME" && (e.relation_type == "ADAPTATION" || e.relation_type == "SOURCE") {
            if anime_adaptation.is_none() {
                anime_adaptation = Some(AnimeAdaptation {
                    anilist_id: e.node.id,
                    title: title.clone(),
                    cover_url: cover.clone(),
                    format: e.node.format.clone(),
                });
            }
        }
        if kind == "MANGA" {
            relations.push(RelatedWork {
                anilist_id: e.node.id,
                relation: e.relation_type,
                title,
                cover_url: cover,
                kind,
                format: e.node.format,
            });
        }
    }

    let recommendations: Vec<Recommended> = media
        .recommendations
        .nodes
        .into_iter()
        .filter_map(|n| n.media_recommendation)
        .filter(|m| m.kind.as_deref() == Some("MANGA"))
        .take(8)
        .map(|m| Recommended {
            anilist_id: m.id,
            title: pick_title(&m.title),
            cover_url: m.cover_image.and_then(|c| c.large.or(c.medium)),
        })
        .collect();

    let bundle = Bundle {
        relations,
        recommendations,
        anime_adaptation,
    };
    cache_put(anilist_id, Some(bundle.clone()));
    Ok(Some(bundle))
}

fn cache_get(id: i64) -> Option<Option<Bundle>> {
    let m = cache().lock().ok()?;
    let (v, t) = m.get(&id)?;
    if t.elapsed() < CACHE_TTL {
        Some(v.clone())
    } else {
        None
    }
}

fn cache_put(id: i64, v: Option<Bundle>) {
    if let Ok(mut m) = cache().lock() {
        m.insert(id, (v, Instant::now()));
    }
}

fn pick_title(t: &GqlTitle) -> String {
    t.english
        .clone()
        .or_else(|| t.romaji.clone())
        .unwrap_or_else(|| "untitled".into())
}

#[derive(Deserialize)]
struct GqlResp {
    data: Option<GqlData>,
}

#[derive(Deserialize)]
struct GqlData {
    #[serde(rename = "Media")]
    media: Option<GqlMedia>,
}

#[derive(Deserialize)]
struct GqlMedia {
    relations: GqlRelations,
    recommendations: GqlRecs,
}

#[derive(Deserialize)]
struct GqlRelations {
    edges: Vec<GqlRelEdge>,
}

#[derive(Deserialize)]
struct GqlRelEdge {
    #[serde(rename = "relationType")]
    relation_type: String,
    node: GqlNode,
}

#[derive(Deserialize)]
struct GqlNode {
    id: i64,
    #[serde(rename = "type")]
    kind: Option<String>,
    format: Option<String>,
    title: GqlTitle,
    #[serde(rename = "coverImage")]
    cover_image: Option<GqlCover>,
}

#[derive(Deserialize)]
struct GqlRecs {
    nodes: Vec<GqlRecNode>,
}

#[derive(Deserialize)]
struct GqlRecNode {
    #[serde(rename = "mediaRecommendation")]
    media_recommendation: Option<GqlRecMedia>,
}

#[derive(Deserialize)]
struct GqlRecMedia {
    id: i64,
    #[serde(rename = "type")]
    kind: Option<String>,
    title: GqlTitle,
    #[serde(rename = "coverImage")]
    cover_image: Option<GqlCover>,
}

#[derive(Deserialize)]
struct GqlTitle {
    romaji: Option<String>,
    english: Option<String>,
}

#[derive(Deserialize)]
struct GqlCover {
    large: Option<String>,
    medium: Option<String>,
}
