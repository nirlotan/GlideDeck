//! Recognising web video links: which site a link belongs to, the video id,
//! and the address of the site's embeddable player.

/// A video site whose player can be embedded in a web page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Site {
    YouTube,
    Vimeo,
    Loom,
}

/// A recognised link to a video on a site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiteVideo {
    pub site: Site,
    pub id: String,
    /// Start offset in seconds (`?t=` / `#t=`), if the link has one.
    pub start: Option<u32>,
}

impl SiteVideo {
    /// The site's embeddable player. YouTube uses its privacy-enhanced domain.
    pub fn embed_url(&self) -> String {
        self.player_url(false, false, false)
    }

    /// The embeddable player with playback options.
    pub fn player_url(&self, autoplay: bool, muted: bool, looping: bool) -> String {
        let mut q = String::new();
        let mut flag = |on: bool, s: &str| {
            if on {
                q.push('&');
                q.push_str(s);
            }
        };
        match self.site {
            Site::YouTube => {
                flag(autoplay, "autoplay=1");
                flag(muted, "mute=1");
                flag(looping, &format!("loop=1&playlist={}", self.id));
                if let Some(t) = self.start {
                    flag(true, &format!("start={t}"));
                }
                format!("https://www.youtube-nocookie.com/embed/{}?rel=0&playsinline=1{q}", self.id)
            }
            Site::Vimeo => {
                flag(autoplay, "autoplay=1");
                flag(muted, "muted=1");
                flag(looping, "loop=1");
                let start = self.start.map(|s| format!("#t={s}s")).unwrap_or_default();
                format!("https://player.vimeo.com/video/{}?dnt=1{q}{start}", self.id)
            }
            Site::Loom => {
                flag(autoplay, "autoplay=1");
                flag(muted, "muted=1");
                format!("https://www.loom.com/embed/{}?hide_owner=true{q}", self.id)
            }
        }
    }

    pub fn site_name(&self) -> &'static str {
        match self.site {
            Site::YouTube => "YouTube",
            Site::Vimeo => "Vimeo",
            Site::Loom => "Loom",
        }
    }
}

/// File extensions of video files (and HLS playlists) that play directly.
const DIRECT_EXTS: &[&str] = &["mp4", "m4v", "mov", "webm", "mkv", "ogv", "m3u8"];

/// True for `http://` and `https://` addresses, the only ones GlideDeck opens.
pub fn is_web_url(s: &str) -> bool {
    let s = s.trim();
    let lower = s.to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://"))
        && !s.chars().any(|c| c.is_whitespace() || c.is_control() || c == '"' || c == '<' || c == '>')
        && host(s).is_some()
}

/// The host part of a web address, lowercased, without `www.` or `m.`.
fn host(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = host.split(':').next()?.to_ascii_lowercase();
    if host.is_empty() || !host.contains('.') {
        return None;
    }
    Some(host.trim_start_matches("www.").trim_start_matches("m.").to_owned())
}

/// The path segments and the query (with any fragment) of a web address.
fn path_and_query(url: &str) -> (Vec<&str>, &str) {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let rest = rest.find('/').map_or("", |i| &rest[i..]);
    let (path, query) = rest.split_once(['?', '#']).unwrap_or((rest, ""));
    (path.split('/').filter(|s| !s.is_empty()).collect(), query)
}

fn query_param<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query.split(['&', '#', '?']).find_map(|kv| kv.split_once('=').filter(|(k, _)| *k == key).map(|(_, v)| v))
}

/// Parses `90`, `90s`, `1m30s` or `1h2m3s`.
fn parse_time(t: &str) -> Option<u32> {
    if let Ok(s) = t.parse::<u32>() {
        return Some(s);
    }
    let (mut total, mut num) = (0u32, String::new());
    for c in t.chars() {
        match c {
            '0'..='9' => num.push(c),
            'h' | 'm' | 's' => {
                let n: u32 = num.parse().ok()?;
                total += n * match c {
                    'h' => 3600,
                    'm' => 60,
                    _ => 1,
                };
                num.clear();
            }
            _ => return None,
        }
    }
    num.is_empty().then_some(total)
}

fn valid_id(id: &str, len: std::ops::RangeInclusive<usize>) -> bool {
    len.contains(&id.len()) && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Recognises YouTube, Vimeo and Loom video links.
pub fn site_video(url: &str) -> Option<SiteVideo> {
    let url = url.trim();
    if !is_web_url(url) {
        return None;
    }
    let host = host(url)?;
    let (segs, query) = path_and_query(url);
    let start = query_param(query, "t").or_else(|| query_param(query, "start")).and_then(parse_time);
    let (site, id) = match host.as_str() {
        "youtube.com" | "music.youtube.com" | "youtube-nocookie.com" => {
            let id = match segs.as_slice() {
                ["watch", ..] => query_param(query, "v")?,
                ["embed" | "shorts" | "live" | "v", id, ..] => id,
                _ => return None,
            };
            (Site::YouTube, id)
        }
        "youtu.be" => (Site::YouTube, *segs.first()?),
        "vimeo.com" => (Site::Vimeo, *segs.iter().rev().find(|s| s.chars().all(|c| c.is_ascii_digit()))?),
        "player.vimeo.com" => match segs.as_slice() {
            ["video", id, ..] => (Site::Vimeo, *id),
            _ => return None,
        },
        "loom.com" => match segs.as_slice() {
            ["share" | "embed", id, ..] => (Site::Loom, *id),
            _ => return None,
        },
        _ => return None,
    };
    let ok = match site {
        Site::YouTube => valid_id(id, 11..=11),
        Site::Vimeo => valid_id(id, 1..=12) && id.chars().all(|c| c.is_ascii_digit()),
        Site::Loom => valid_id(id, 16..=64),
    };
    ok.then(|| SiteVideo { site, id: id.to_owned(), start })
}

/// True for a direct link to a video file (`…/clip.mp4`, an HLS playlist…).
pub fn is_direct_video(url: &str) -> bool {
    if !is_web_url(url) {
        return false;
    }
    let (segs, _) = path_and_query(url.trim());
    segs.last()
        .and_then(|last| last.rsplit_once('.'))
        .is_some_and(|(_, ext)| DIRECT_EXTS.contains(&ext.to_ascii_lowercase().as_str()))
}

/// True when a pasted or typed link should become a web video.
pub fn is_video_link(url: &str) -> bool {
    site_video(url).is_some() || is_direct_video(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_sites_and_ids() {
        let yt = |u: &str| site_video(u).map(|v| (v.site, v.id, v.start));
        let id = "dQw4w9WgXcQ".to_owned();
        assert_eq!(yt("https://www.youtube.com/watch?v=dQw4w9WgXcQ"), Some((Site::YouTube, id.clone(), None)));
        assert_eq!(yt("https://youtu.be/dQw4w9WgXcQ?t=42"), Some((Site::YouTube, id.clone(), Some(42))));
        assert_eq!(yt("https://m.youtube.com/watch?feature=share&v=dQw4w9WgXcQ&t=1m5s"), Some((Site::YouTube, id.clone(), Some(65))));
        assert_eq!(yt("https://www.youtube.com/shorts/dQw4w9WgXcQ"), Some((Site::YouTube, id.clone(), None)));
        assert_eq!(yt("https://vimeo.com/channels/staffpicks/76979871"), Some((Site::Vimeo, "76979871".into(), None)));
        assert_eq!(
            yt("https://www.loom.com/share/0281766fa2d04bb788eaf19e65135184"),
            Some((Site::Loom, "0281766fa2d04bb788eaf19e65135184".into(), None))
        );
        assert_eq!(yt("https://www.youtube.com/watch?v=bad"), None);
        assert_eq!(yt("https://example.com/watch?v=dQw4w9WgXcQ"), None);
        assert_eq!(yt("javascript:alert(1)//youtube.com/watch?v=dQw4w9WgXcQ"), None);
        let v = site_video("https://youtu.be/dQw4w9WgXcQ?t=42").unwrap();
        assert_eq!(v.embed_url(), "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ?rel=0&playsinline=1&start=42");
        assert_eq!(
            v.player_url(true, true, true),
            "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ?rel=0&playsinline=1&autoplay=1&mute=1&loop=1&playlist=dQw4w9WgXcQ&start=42"
        );
        let vimeo = site_video("https://vimeo.com/76979871#t=30").unwrap();
        assert_eq!(vimeo.player_url(true, false, false), "https://player.vimeo.com/video/76979871?dnt=1&autoplay=1#t=30s");
    }

    #[test]
    fn direct_links_and_schemes() {
        assert!(is_direct_video("https://cdn.example.com/media/clip.MP4?token=1"));
        assert!(is_direct_video("https://example.com/live/index.m3u8"));
        assert!(!is_direct_video("https://example.com/page.html"));
        assert!(!is_direct_video("file:///C:/clip.mp4"));
        assert!(!is_web_url("https://exa mple.com/x"));
        assert!(!is_web_url("https://localhost/x.mp4"));
        assert!(is_video_link("https://youtu.be/dQw4w9WgXcQ"));
    }
}
