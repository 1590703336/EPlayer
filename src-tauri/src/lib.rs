// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use serde::Deserialize;
use serde::Serialize;
use std::time::Duration;
use tauri_plugin_log::{Target, TargetKind};
use youtube_captions::format::Format;
use youtube_captions::language_tags::LanguageTag;
use youtube_captions::{CaptionScraper, Digest, DigestScraper};
use openai_api_rust::*;
use openai_api_rust::chat::*;
use openai_api_rust::completions::*;
use std::process::Command;
use std::path::Path;
use base64::encode;
use std::io::Read;
use uuid::Uuid;

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

#[derive(Serialize, Deserialize)]
struct OpenAIRequest {
    model: String,
    prompt: String,
    max_tokens: u32,
}

#[derive(Serialize, Deserialize)]
struct OpenAIResponse {
    choices: Vec<Choice>,
}

#[derive(Serialize, Deserialize)]
struct Choice {
    text: String,
}

#[derive(Clone, Serialize, Deserialize)]
enum AssistantRole {
    Word_Dictionary,
    Word_More,
    Word_Etymology,
    Word_Example,
    Word_Custom,
    Word_Symbols,
    Sentence_Translation,
    Sentence_Structure,
    Sentence_Copy,
    Sentence_Example,
    Sentence_Transform,
    Sentence_Custom,
}

impl AssistantRole {
    fn get_system_prompt(&self) -> String {
        match self {
            AssistantRole::Word_Dictionary => "Provides parts of speech, Chinese translation and clear definition suitable for English learners less than 20 words. Output format: [part of speech],[Chinese translation],[definition]".to_string(),
            AssistantRole::Word_Symbols => "Provide English and American pronunciation symbols. Output format: [English symbol],[American symbol]".to_string(),
            AssistantRole::Word_More => "Provide one synonyms and additional notes about usage,including whether the word is formal, or used in specific contexts, less than 20 words. Output format: [synonym],[notes]".to_string(),
            AssistantRole::Word_Etymology => "Provide etymology or origin, less than 20 words. Output format: [etymology or origin]".to_string(),
            AssistantRole::Word_Example => "Provide one example sentences of less than 10 words. Output format: [example sentence]".to_string(),
            AssistantRole::Word_Custom => "Provide results upon request briefly, less than 20 words. Choose the language of your reply for English learners.".to_string(),
            AssistantRole::Sentence_Translation => "Provide a clear, concise Chinese translation of the text, less than 20 words. Output format: [translation]".to_string(),
            AssistantRole::Sentence_Structure => "Provide a breakdown of the grammatical structure, identifying key parts like the subject, verb, and object. Output format: [structure]".to_string(),
            AssistantRole::Sentence_Copy => "Provide one similar expressions, less than 20 words. Output format: [similar expressions]".to_string(),
            AssistantRole::Sentence_Example => "Provide one example sentences or phrases for the content, in less than 15 words. Output format: [example sentence]".to_string(),
            AssistantRole::Sentence_Transform => "Provide a transformation of the sentence, like formal, informal, or suitable for specific situations,less than 30 words. Output format: [transformation]".to_string(),
            AssistantRole::Sentence_Custom => "Provide results upon request briefly, less than 20 words. Choose the language of your reply for English learners.".to_string(),
        }
    }
}

#[derive(Serialize)]
struct AIResponse {
    content: String,
    input_tokens: u32,
    output_tokens: u32,
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
async fn communicate_with_openai(prompt: String, role: AssistantRole) -> Result<AIResponse, String> {
    let auth = Auth::from_env().map_err(|e| format!("API密钥错误: {:?}", e))?;
    let openai = OpenAI::new(auth, "https://api.openai.com/v1/");
    
    let body = ChatBody {
        model: "gpt-4o-mini-2024-07-18".to_string(),
        max_tokens: Some(100),
        temperature: Some(0_f32),
        top_p: Some(0_f32),
        n: Some(2),
        stream: Some(false),
        stop: None,
        presence_penalty: None,
        frequency_penalty: None,
        logit_bias: None,
        user: None,
        messages: vec![
            Message {
                role: Role::System,
                content: role.get_system_prompt()
            },
            Message {
                role: Role::User,
                content: prompt
            }
        ],
    };
    
    let rs = openai.chat_completion_create(&body)
        .map_err(|e| format!("OpenAI API 调用失败: {:?}", e))?;

    let message = rs.choices
        .first()
        .and_then(|choice| choice.message.as_ref())
        .ok_or("未收到有效的回复")?;

    Ok(AIResponse {
        content: message.content.clone(),
        input_tokens: rs.usage.prompt_tokens.unwrap_or(0),
        output_tokens: rs.usage.completion_tokens.unwrap_or(0),
    })
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn extract_audio(video_path: String) -> Result<String, String> {
    let ffmpeg_path = get_ffmpeg_path()?;
    println!("接收到的数据前缀: {}", &video_path[..50]);
    
    // 使用简短的临文件名
    let temp_input = format!("in_{}.mp4", Uuid::new_v4().simple());
    
    if video_path.starts_with("data:") {
        let base64_data = video_path.split("base64,").nth(1)
            .ok_or("无效的data URL格式")?;
            
        // 解码base64数据
        let video_data = base64::decode(base64_data)
            .map_err(|e| format!("base64解码失败: {}", e))?;
            
        // 写入临时文件
        std::fs::write(&temp_input, video_data)
            .map_err(|e| format!("写入临时文件失败: {}", e))?;
    } else {
        // 如果是文件路径，直接使用
        std::fs::copy(&video_path, &temp_input)
            .map_err(|e| format!("复制文件失败: {}", e))?;
    }

    let output = Command::new(ffmpeg_path)
        .args(&[
            "-i", &temp_input,
            "-vn",
            "-acodec", "mp3",
            "-f", "mp3",
            "out.mp3"
        ])
        .output()
        .map_err(|e| format!("ffmpeg执行失败: {}", e))?;

    // 清理临时文件
    let _ = std::fs::remove_file(&temp_input);

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(format!("音频提取失败: {}", error));
    }

    // 读取输出文件
    let mut file = std::fs::File::open("out.mp3")
        .map_err(|e| format!("无法读取输出文件: {}", e))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|e| format!("读取文件失败: {}", e))?;

    // 清理输出文件
    let _ = std::fs::remove_file("out.mp3");

    // 转换为base64
    let base64_audio = encode(&buffer);
    Ok(format!("data:audio/mp3;base64,{}", base64_audio))
}

fn get_ffmpeg_path() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        let ffmpeg = "ffmpeg.exe";
        if let Ok(output) = Command::new("where").arg(ffmpeg).output() {
            if output.status.success() {
                return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let ffmpeg = "ffmpeg";
        if let Ok(output) = Command::new("which").arg(ffmpeg).output() {
            if output.status.success() {
                return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
            }
        }
    }

    Err("找不到ffmpeg，请确保已安装并添加到系统PATH中".to_string())
}

#[derive(Serialize, Deserialize)]
struct WhisperResponse {
    text: String,
    segments: Vec<WhisperSegment>,
}

#[derive(Serialize, Deserialize)]
struct WhisperSegment {
    start: f64,
    end: f64,
    text: String,
}

#[tauri::command]
async fn transcribe_audio(audio_base64: String) -> Result<Vec<Subtitle>, String> {
    let auth = Auth::from_env().map_err(|e| format!("API密钥错误: {:?}", e))?;
    let client = reqwest::Client::new();

    // 从base64中提取实际的音频数据
    let audio_data = audio_base64
        .split("base64,")
        .nth(1)
        .ok_or("无效的音频数据格式")?;
    
    let audio_bytes = base64::decode(audio_data)
        .map_err(|e| format!("解码音频数据失败: {}", e))?;

    // 创建multipart form
    let form = reqwest::multipart::Form::new()
        .part("file", reqwest::multipart::Part::bytes(audio_bytes)
            .file_name("audio.mp3")
            .mime_str("audio/mp3")
            .map_err(|e| format!("创建表单失败: {}", e))?)
        .text("model", "whisper-1")
        .text("language", "zh")
        .text("response_format", "verbose_json")  // 请求详细的JSON响应
        .text("timestamp_granularities", "segment");  // 请求分段时间戳

    // 发送请求到Whisper API
    let response = client
        .post("https://api.openai.com/v1/audio/transcriptions")
        .header("Authorization", format!("Bearer {}", auth.api_key))
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("API请求失败: {}", e))?;

    // 解析响应
    let whisper_response = response
        .json::<WhisperResponse>()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    // 将Whisper响应转换为字幕格式
    let subtitles: Vec<Subtitle> = whisper_response.segments
        .into_iter()
        .enumerate()
        .map(|(i, segment)| Subtitle {
            id: (i + 1) as u32,
            text: segment.text,
            startSeconds: segment.start,
            endSeconds: segment.end,
        })
        .collect();

    Ok(subtitles)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()        
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            greet, 
            get_transcript, 
            communicate_with_openai,
            extract_audio,
            transcribe_audio  // 添加新命令
        ])
        .plugin(tauri_plugin_log::Builder::new().build())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
