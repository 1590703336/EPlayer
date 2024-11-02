// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use serde::Deserialize;
use serde::Serialize;
use std::time::Duration;
use tauri_plugin_log::{Target, TargetKind};
use youtube_captions::format::Format;
use youtube_captions::language_tags::LanguageTag;
use youtube_captions::{CaptionScraper, Digest, DigestScraper};

const LANGUAGES: [&'static str; 8] = ["en", "zh-TW", "ja", "zh-Hant", "ko", "zh", "es", "fr"];  //英语、繁体中文、日语、韩语、简体中文、西班牙语、法语

#[derive(Deserialize)]
struct Transcript {
    events: Vec<Event>,
}

#[derive(Deserialize)]
struct Event {
    segs: Option<Vec<Segment>>,
    tStartMs: Option<f64>,
    dDurationMs: Option<f64>,
}

#[derive(Deserialize)]
struct Segment {
    utf8: String,
}

#[derive(Serialize)]
struct Subtitle {
    id: u32,
    text: String,
    startSeconds: f64,
    endSeconds: f64,
}

#[tauri::command]
async fn get_transcript(video: String) -> Vec<Subtitle> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .unwrap();
    let digest = DigestScraper::new(client);

    // Fetch the video
    let scraped = fetch_video(video, digest).await;

    // Find our preferred language, the priority is the order of LANGUAGES
    let language = get_caption_language(&scraped).unwrap();
    let captions = scraped
        .captions
        .iter()
        .find(|caption| caption.lang_tag == language)
        .unwrap();

    let transcript_json = captions.fetch(Format::JSON3).await.unwrap();

    let root: Transcript = serde_json::from_str(transcript_json.as_str()).unwrap();

    // Collect all utf8 fields from all events and all segments
    let mut subtitles: Vec<Subtitle> = root
        .events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            let start_seconds = event.tStartMs.map(|t| t / 1000.0)?;
            let end_seconds = event.dDurationMs.map(|d| start_seconds + d / 1000.0)?;
            event.segs.as_ref().map(|segs| {
                segs.iter()
                    .map(|segment| Subtitle {
                        id: (index + 1) as u32,
                        text: segment.utf8.clone(),
                        startSeconds: start_seconds,
                        endSeconds: end_seconds,
                    })
                    .collect::<Vec<Subtitle>>()
            })
        })
        .flatten()
        .collect();    

    // 检查如果前两条字幕的开始时间相同，则属于自动生成字幕，需要合并字幕
    if subtitles.len() >= 10 && subtitles[..10].windows(2).any(|w| w[0].startSeconds == w[1].startSeconds || w[0].endSeconds == w[1].endSeconds) {
        // 处理包含换行符的字幕
        let mut merged_subtitles = Vec::new();
        let mut current_text = String::new();
        let mut current_start = -1.0;
        let mut current_id = 1;

        for subtitle in subtitles {
            if subtitle.text.contains('\n') {
                // 当前字幕包含换行符,表示一行字幕结束
                current_text.push_str(&subtitle.text);
                merged_subtitles.push(Subtitle {
                    id: current_id,
                    text: current_text,
                    startSeconds: current_start,
                    endSeconds: -1.0,
                });
                if current_id > 1 {
                    merged_subtitles[current_id as usize - 2].endSeconds = current_start;
                }
                current_text = String::new();
                current_start = -1.0;
                current_id += 1;
            } else {
                // 累积文本
                if current_start == -1.0 {
                    current_start = subtitle.startSeconds;
                }
                current_text.push_str(&subtitle.text);
            }
        }

        // 移除所有换行符
        merged_subtitles.iter_mut().for_each(|s| {
            s.text = s.text.replace('\n', " ");
        });

        // 输出合并后的字幕
        println!("\n合并后的字幕：");
        for subtitle in &merged_subtitles {
            println!("ID: {}, Time: {}-{}, Text: {}", 
                subtitle.id,
                subtitle.startSeconds,
                subtitle.endSeconds,
                subtitle.text
            );
        }

        merged_subtitles
    } else {
        // 如果没有换行符，直接返回原始字幕
        subtitles
    }
}

fn get_caption_language(scraped: &Digest) -> Option<LanguageTag> {
    for lang in LANGUAGES.iter() {
        let language = LanguageTag::parse(lang).unwrap();
        if scraped
            .captions
            .iter()
            .any(|caption| language.matches(&caption.lang_tag))
        {
            return Some(language);
        }
    }
    None
}

fn find_preferred_language() -> Option<LanguageTag> {
    let mut language = None;

    for lang in LANGUAGES {
        match LanguageTag::parse(lang) {
            Ok(result) => {
                language = Some(result);
                break;
            }
            Err(_) => continue,
        }
    }
    language
}

async fn fetch_video(video: String, digest: DigestScraper) -> Digest {
    let mut scraped = None;

    for lang in LANGUAGES {
        match digest.fetch(&video, lang).await {
            Ok(result) => {
                scraped = Some(result);
                break;
            }
            Err(_) => continue,
        }
    }

    let scraped = scraped.unwrap();
    scraped
}


#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()        
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![greet, get_transcript])
        .plugin(tauri_plugin_log::Builder::new().build())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
