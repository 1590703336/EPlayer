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
use std::path::PathBuf;

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
    
    // 使用系统临时目录
    let temp_dir = std::env::temp_dir();
    let uuid = Uuid::new_v4();
    
    // 创建临时文件路径
    let temp_input = temp_dir.join(format!("input_{}.mp4", uuid));
    let temp_output = temp_dir.join(format!("output_{}.mp3", uuid));

    // 将路径转换为字符串
    let temp_input_str = temp_input.to_string_lossy().to_string();
    let temp_output_str = temp_output.to_string_lossy().to_string();

    println!("输入文件路径: {}", temp_input_str);
    println!("输出文件路径: {}", temp_output_str);

    if video_path.starts_with("data:") {
        let base64_data = video_path.split("base64,").nth(1)
            .ok_or("无效的data URL格式")?;
            
        let video_data = base64::decode(base64_data)
            .map_err(|e| format!("base64解码失败: {}", e))?;
            
        // 使用 PathBuf 的路径写入文件
        std::fs::write(&temp_input, video_data)
            .map_err(|e| format!("写入临时文件失败: {}", e))?;
    } else {
        return Err("不支持的视频格式".to_string());
    }

    // 执行 ffmpeg 命令，不需要手动添加引号
    let output = Command::new(&ffmpeg_path)
        .args(&[
            "-i",
            &temp_input_str,  // 直接使用路径字符串
            "-vn",
            "-acodec",
            "mp3",
            "-f",
            "mp3",
            &temp_output_str  // 直接使用路径字符串
        ])
        .output()
        .map_err(|e| format!("ffmpeg执行失败: {}", e))?;

    // 检查命令执行结果
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(format!("音频提取失败: {}", error));
    }

    // 读取输出文件
    let mut file = std::fs::File::open(&temp_output)
        .map_err(|e| format!("无法读取输出文件: {}", e))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|e| format!("读取文件失败: {}", e))?;

    // 清理临时文件
    let _ = std::fs::remove_file(&temp_input);
    let _ = std::fs::remove_file(&temp_output);

    // 转换为base64
    let base64_audio = base64::encode(&buffer);
    Ok(format!("data:audio/mp3;base64,{}", base64_audio))
}

fn get_ffmpeg_path() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        let output = Command::new("where")
            .arg("ffmpeg.exe")
            .output()
            .map_err(|_| "找不到 ffmpeg，请确保已安装并添加到系统 PATH 中".to_string())?;

        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout);
            Ok(path.lines().next().unwrap_or("ffmpeg.exe").trim().to_string())
        } else {
            Err("找不到 ffmpeg，请确保已安装并添加到系统 PATH 中".to_string())
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let output = Command::new("which")
            .arg("ffmpeg")
            .output()
            .map_err(|_| "找不到 ffmpeg，请确保已安装并添加到系统 PATH 中".to_string())?;

        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout);
            Ok(path.trim().to_string())
        } else {
            Err("找不到 ffmpeg，请确保已安装并添加到系统 PATH 中".to_string())
        }
    }
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

#[derive(Serialize)]
struct TranscriptionResult {
    subtitles: Vec<Subtitle>,
    duration: f64,  // 添加音频时长字段
}

#[tauri::command]
async fn transcribe_audio(audio_base64: String, language: String) -> Result<TranscriptionResult, String> {
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
        .text("language", language)
        .text("response_format", "verbose_json")
        .text("timestamp_granularities", "segment");

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
        .iter()
        .enumerate()
        .map(|(i, segment)| Subtitle {
            id: (i + 1) as u32,
            text: segment.text.clone(),
            startSeconds: segment.start,
            endSeconds: segment.end,
        })
        .collect();

    // 计算总时长（使用最后一个片段的结束时间）
    let duration = whisper_response.segments
        .last()
        .map(|segment| segment.end)
        .unwrap_or(0.0);

    Ok(TranscriptionResult {
        subtitles,
        duration,
    })
}

// 添加新的结构体用于字幕搜索结果
#[derive(Serialize)]
struct SubtitleSearchResult {
    file_name: String,
    language: String,
    download_url: String,
    file_id: String,  // 添加 file_id 用于下载
}

// 添加搜索字幕的命令
#[tauri::command]
async fn search_subtitles(file_name: String) -> Result<Vec<SubtitleSearchResult>, String> {
    let client = reqwest::Client::new();
    let api_key = std::env::var("OPENSUBTITLES_API_KEY")
        .map_err(|_| "OpenSubtitles API key not found".to_string())?;

    println!("搜索字幕: {}", file_name);

    // 发送请求
    let response = client
        .get("https://api.opensubtitles.com/api/v1/subtitles")
        .header("Api-Key", api_key)
        .header("User-Agent", "EPlayer v1.0") // 添加 User-Agent
        .query(&[
            ("query", file_name.as_str()),
            ("languages", "en,zh"),
        ])
        .send()
        .await
        .map_err(|e| format!("搜索字幕失败: {}", e))?;

    // 检查响应状态
    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await
            .unwrap_or_else(|_| "无法读取错误信息".to_string());
        return Err(format!("API 返回错误: {} - {}", status, error_text));
    }

    // 解析 JSON
    let results: serde_json::Value = response.json()
        .await
        .map_err(|e| format!("解析 JSON 失败: {}", e))?;

    println!("搜索结果数量: {}", results["total_count"]);

    // 解析搜索结果
    let subtitles = results["data"]
        .as_array()
        .ok_or("无效的响应格式: 找不到 data 数组")?
        .iter()
        .filter_map(|item| {
            let attributes = item.get("attributes")?;
            
            // 获取文件信息
            Some(SubtitleSearchResult {
                file_name: attributes.get("release")?
                    .as_str()?
                    .to_string(),
                language: attributes.get("language")?
                    .as_str()?
                    .to_string(),
                download_url: format!("https://www.opensubtitles.com/en/subtitles/{}",
                    attributes.get("slug")?
                        .as_str()?),
                file_id: attributes.get("subtitle_id")?
                    .as_str()?
                    .to_string(),
            })
        })
        .collect();

    Ok(subtitles)
}

// 添加下载字幕的命令
#[tauri::command]
async fn download_subtitle(file_id: String) -> Result<String, String> {
    let client = reqwest::Client::new();
    let api_key = std::env::var("OPENSUBTITLES_API_KEY")
        .map_err(|_| "OpenSubtitles API key not found".to_string())?;

    println!("开始下载字幕, file_id: {}", file_id);

    // 首先获取下载链接
    let response = client
        .post("https://api.opensubtitles.com/api/v1/download")
        .header("Api-Key", api_key)
        .header("User-Agent", "EPlayer v1.0")  // 添加 User-Agent
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")   // 添加 Accept 头
        .json(&serde_json::json!({
            "file_id": file_id,
            "sub_format": "srt"  // 指定字幕格式
        }))
        .send()
        .await
        .map_err(|e| format!("获取下载链接失败: {}", e))?;

    // 检查响应状态
    if !response.status().is_success() {
        let status = response.status();  // 先保存状态码
        let error_text = response.text().await
            .unwrap_or_else(|_| "无法读取错误信息".to_string());
        return Err(format!("API 返回错误: {} - {}", status, error_text));
    }

    // 打印响应内容用于调试
    let response_text = response.text().await
        .map_err(|e| format!("读取响应内容失败: {}", e))?;
    println!("下载链接响应: {}", response_text);

    // 解析 JSON 响应
    let download_info: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| format!("解析下载信息失败: {} - 响应内容: {}", e, response_text))?;

    // 获取下载链接
    let download_url = download_info["link"]
        .as_str()
        .ok_or_else(|| format!("无效的下载链接 - 响应内容: {}", response_text))?;

    println!("获取到下载链接: {}", download_url);

    // 下载字幕文件
    let subtitle_response = client
        .get(download_url)
        .send()
        .await
        .map_err(|e| format!("下载字幕失败: {}", e))?;

    // 检查下载响应状态
    let status = subtitle_response.status();
    if !status.is_success() {
        let error_text = subtitle_response.text().await
            .unwrap_or_else(|_| "无法读取错误信息".to_string());
        return Err(format!("下载字幕失败: {} - {}", status, error_text));
    }

    // 读取字幕内容
    let subtitle_content = subtitle_response.text().await
        .map_err(|e| format!("读取字幕内容失败: {}", e))?;

    Ok(subtitle_content)
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
            transcribe_audio,
            search_subtitles,  // 添加新命令
            download_subtitle  // 添加新命令
        ])
        .plugin(tauri_plugin_log::Builder::new().build())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
