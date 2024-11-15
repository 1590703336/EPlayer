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
use reqwest::Client;

use md5::{Md5, Digest as Md5Digest};

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

#[derive(Serialize)]
struct ProxyRequest {
    prompt: String,
    role: String,
}

#[derive(Deserialize)]
struct ProxyResponse {
    content: String,
    input_tokens: usize,
    output_tokens: usize,
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
    let client = Client::new();
    let url = "https://eplayer-server.vercel.app/api/openai";
    // let url = "http://localhost:3000/api/openai";

    // 构建请求体，将角色信息传给代理
    let request_body = ProxyRequest {
        prompt,
        role: role.get_system_prompt() 
    };
  

    // 向 Vercel API 发送 POST 请求
    let response = client
        .post(url)
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("请求失败: {:?}", e))?;

    if response.status().is_success() {
        let proxy_response: ProxyResponse = response
            .json()
            .await
            .map_err(|e| format!("解析响应失败: {:?}", e))?;

        Ok(AIResponse {
            content: proxy_response.content,
            input_tokens: proxy_response.input_tokens as u32,
            output_tokens: proxy_response.output_tokens as u32,
        })
    } else {
        let error_message = response
            .text()
            .await
            .unwrap_or_else(|_| "未收到有效的响应".to_string());
        Err(format!("代理 API 调用失败: {}", error_message))
    }
}


// #[tauri::command]
// async fn communicate_with_openai(prompt: String, role: AssistantRole) -> Result<AIResponse, String> {
//     let auth = Auth::from_env().map_err(|e| format!("API密钥错误: {:?}", e))?;
//     let openai = OpenAI::new(auth, "https://api.openai.com/v1/");
    
//     let body = ChatBody {
//         model: "gpt-4o-mini-2024-07-18".to_string(),
//         max_tokens: Some(100),
//         temperature: Some(0_f32), // 降低温度以获得更稳定的结果
//         top_p: Some(0_f32),
//         n: Some(2),
//         stream: Some(false),
//         stop: None,
//         presence_penalty: None,
//         frequency_penalty: None,
//         logit_bias: None,
//         user: None,
//         messages: vec![
//             Message {
//                 role: Role::System,
//                 content: role.get_system_prompt()
//             },
//             Message {
//                 role: Role::User,
//                 content: prompt
//             }
//         ],
//     };
    
//     let rs = openai.chat_completion_create(&body)
//         .map_err(|e| format!("OpenAI API 调用失败: {:?}", e))?;

//     let message = rs.choices
//         .first()
//         .and_then(|choice| choice.message.as_ref())
//         .ok_or("未收到有效的回复")?;

//     Ok(AIResponse {
//         content: message.content.clone(),
//         input_tokens: rs.usage.prompt_tokens.unwrap_or(0),
//         output_tokens: rs.usage.completion_tokens.unwrap_or(0),
//     })
// }

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn extract_audio(video_path: String) -> Result<String, String> {
    let ffmpeg_path = get_ffmpeg_path()?;
    println!("ffmpeg路径: {}", ffmpeg_path);
    
    // 使用系统临时目录
    let temp_dir = std::env::temp_dir();
    let uuid = Uuid::new_v4();
    
    // 创建临时文件路径
    let temp_input = temp_dir.join(format!("input_{}.mp4", uuid));
    let temp_output = temp_dir.join(format!("output_{}.mp3", uuid));

    // 将路径换为字符串
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
            "-hwaccel", "auto", // 自动选择可用的硬件加速
            "-i",
            &temp_input_str,
            "-vn",
            "-acodec",
            "mp3",
            "-f",
            "mp3", 
            "-threads",
            "0",
            &temp_output_str
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
        .map_err(|e| format!("读取文件败: {}", e))?;

    // 清理临时文件
    let _ = std::fs::remove_file(&temp_input);
    let _ = std::fs::remove_file(&temp_output);

    // 转换为base64
    let base64_audio = base64::encode(&buffer);
    Ok(format!("data:audio/mp3;base64,{}", base64_audio))
}

fn get_ffmpeg_path() -> Result<String, String> {
    #[cfg(debug_assertions)]  // 开发模式
    {
        let current_dir = std::env::current_dir()
            .map_err(|_| "无法获取当前目录".to_string())?;
            
        #[cfg(target_os = "windows")] // 添加windows条件
        let ffmpeg_name = "ffmpeg-x86_64-pc-windows-msvc.exe";
        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        let ffmpeg_name = "ffmpeg-x86_64-apple-darwin";
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        let ffmpeg_name = "ffmpeg-aarch64-apple-darwin";
        #[cfg(target_os = "linux")] // 添加x86_64 linux条件
        let ffmpeg_name = "ffmpeg-x86_64-unknown-linux-gnu";

        // 尝试在 binaries 目录查找
        let binaries_path = current_dir            
            .join("binaries")
            .join(ffmpeg_name);

        println!("binaries路径: {}", binaries_path.to_string_lossy());
            
        if binaries_path.exists() {
            return Ok(binaries_path.to_string_lossy().to_string());
        }

        Err("开发模式下找不到 ffmpeg".to_string())
    }

    #[cfg(not(debug_assertions))]  // 发布模式
    {
        let current_exe = std::env::current_exe()
            .map_err(|_| "无法获取当前程序路径".to_string())?;
        let app_dir = current_exe.parent()
            .ok_or("无法获取程序目录".to_string())?;

        #[cfg(target_os = "windows")]
        let ffmpeg_path = app_dir.join("ffmpeg.exe");
        #[cfg(not(target_os = "windows"))]
        let ffmpeg_path = app_dir.join("ffmpeg");

        if ffmpeg_path.exists() {
            #[cfg(not(target_os = "windows"))]
            {
                // 在 Unix 系统上设置执行权限
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&ffmpeg_path, std::fs::Permissions::from_mode(0o755))
                    .map_err(|e| format!("设置执行权限失败: {}", e))?;
            }
            
            Ok(ffmpeg_path.to_string_lossy().to_string())
        } else {
            Err("找不到 ffmpeg，请确保程序目录下存在 ffmpeg".to_string())
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
        .map_err(|e| format!("解码音频据失败: {}", e))?;

    // 创建multipart form
    let prompt = match language.as_str() {
        "en" => "Please segment based on complete sentences and natural speech pauses. Avoid breaking mid-sentence.",
        "zh" => "请严格按照完整句子和自然停顿分段。避免在句子中间断开。",
        "ja" => "文章の完全な意味と自な休止点に基づいて分割してください。文の途中で区切らないでください。",
        _ => "Please segment based on complete sentences and natural speech pauses.",
    };

    let form = reqwest::multipart::Form::new()
        .part("file", reqwest::multipart::Part::bytes(audio_bytes)
            .file_name("audio.mp3")
            .mime_str("audio/mp3")
            .map_err(|e| format!("创建表单失: {}", e))?)
        .text("model", "whisper-1")
        .text("language", language)
        .text("response_format", "verbose_json")
        .text("timestamp_granularities", "segment")
        .text("prompt", prompt)
        .text("temperature", "0.1") // 降低温度以获得更稳定的结果
        //.text("compression_ratio_threshold", "3.0") // 增加压缩比阈值
        //.text("no_speech_threshold", "0.4") // 增加静音阈值
        //.text("compression_ratio_threshold", "2.0")
        //.text("no_speech_threshold", "0.2")
        .text("condition_on_previous_text", "true") // 启用条件文本
        .text("vad_filter", "true"); // 启用VAD过滤
        

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
            startSeconds: (segment.start * 100.0).round() / 100.0,  // 保留两位小数
            endSeconds: (segment.end * 100.0).round() / 100.0,      // 保留两位小数
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
            let files = attributes.get("files")?.as_array()?;
            
            if let Some(file) = files.first() {
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
                    file_id: file.get("file_id")?  // 从 files 数组中获取 file_id
                        .to_string()
                        .replace("\"", "")  // 移除可��的引号
                })
            } else {
                None
            }
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
        .map_err(|e| format!("取下载链接失败: {}", e))?;

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

#[tauri::command]
async fn calculate_md5(video_base64: String) -> Result<String, String> {
    // 从base64中提取实际的视频数据
    let video_data = video_base64
        .split("base64,")
        .nth(1)
        .ok_or("无效的视频数据格式")?;
    
    let video_bytes = base64::decode(video_data)
        .map_err(|e| format!("解码视频数据失败: {}", e))?;

    // 计算MD5
    let mut hasher = Md5::new();
    hasher.update(&video_bytes);
    let result = hasher.finalize();
    
    // 将结果转换为十六进制字符串
    Ok(format!("{:x}", result))
}

// 添加新的结构体用于用户注册
#[derive(Serialize, Deserialize)]
struct User {
    username: String,
    email: String,
    password: String,
    native_language: String,
}

#[derive(Serialize)]
struct RegisterResponse {
    success: bool,
    message: String,
    user_id: Option<String>,  // 合并两个结构体的所有必要字段
}

// 添加注册用户的命令
#[tauri::command]
async fn register_user(
    username: String,
    email: String,
    password: String,
    native_language: String
) -> Result<RegisterResponse, String> {
    let client = Client::new();
    //let url = "https://eplayer-server.vercel.app/api/user";
    let url = "http://localhost:3000/api/user";

    // 构建请求体
    let request_body = RegisterRequest {
        email: email.clone(),
        password: password.clone(),
        native_language: native_language.clone(),
    };

    // 发送请求到 Vercel API
    let response = client
        .post(url)
        .header("Content-Type", "application/json")
        .query(&[
            ("email", email),
            ("password", password),
            ("native_language", native_language),
        ])
        .send()
        .await
        .map_err(|e| format!("注册请求失败: {:?}", e))?;

    // 先获取响应状态码
    let status = response.status();
    
    // 获取响应文本
    let response_text = response.text().await
        .map_err(|e| format!("读取响应内容失败: {:?}", e))?;

    println!("API Response: {}", response_text); // 添加调试日志

    // 尝试解析响应
    if status.is_success() {
        match serde_json::from_str::<RegisterApiResponse>(&response_text) {
            Ok(api_response) => {
                Ok(RegisterResponse {
                    success: api_response.success,
                    message: api_response.message,
                    user_id: api_response.id,
                })
            },
            Err(e) => {
                Err(format!("解析成功响应失败: {} - 响应内容: {}", e, response_text))
            }
        }
    } else {
        // 尝试解析错误响应
        match serde_json::from_str::<RegisterApiResponse>(&response_text) {
            Ok(error_response) => {
                Ok(RegisterResponse {
                    success: false,
                    message: error_response.message,
                    user_id: None,
                })
            },
            Err(_) => {
                // 如果无法解析为 JSON，直接返回响应文本作为错误消息
                Ok(RegisterResponse {
                    success: false,
                    message: response_text,
                    user_id: None,
                })
            }
        }
    }
}

// 用于发送到 Vercel API 的注册请求结构体
#[derive(Serialize)]
struct RegisterRequest {
    email: String,
    password: String,
    native_language: String,
}

// 从 Vercel API 接收的响应结构体
#[derive(Deserialize)]
struct RegisterApiResponse {
    success: bool,
    id: Option<String>,
    message: String,
}

// 添加更新用户信息的请求和响应结构体
#[derive(Serialize)]
struct UpdateUserRequest {
    version: String,
}

#[derive(Serialize, Deserialize)]
struct UpdateUserResponse {
    success: bool,
    message: String,
    data: Option<serde_json::Value>,
}

// 添加更新用户信息的命令
#[tauri::command]
async fn update_user_version(user_id: String, version: String) -> Result<UpdateUserResponse, String> {
    let client = Client::new();
    //let url = "https://eplayer-server.vercel.app/api/user";
    let url = "http://localhost:3000/api/user";

    // 发送 PUT 请求到 Vercel API
    let response = client
        .put(url)
        .header("Content-Type", "application/json")
        .query(&[
            ("id", &user_id),
            ("version", &version),
        ])
        .send()
        .await
        .map_err(|e| format!("更新请求失败: {:?}", e))?;

    // 获取响应文本
    let response_text = response.text().await
        .map_err(|e| format!("读取响应内容失败: {:?}", e))?;

    println!("Update API Response: {}", response_text); // 添加调试日志

    // 尝试解析响应
    match serde_json::from_str::<UpdateUserResponse>(&response_text) {
        Ok(response) => Ok(response),
        Err(e) => Err(format!("解析响应失败: {} - 响应内容: {}", e, response_text))
    }
}

// 添加登录相关的结构体
#[derive(Deserialize)]
struct LoginApiResponse {
    success: bool,
    data: Option<serde_json::Value>,
    message: Option<String>,  // 改为可选字段
}

#[derive(Serialize)]
struct LoginResponse {
    success: bool,
    message: Option<String>,  // 改为可选字段
    user_data: Option<serde_json::Value>,
}

// 添加登录命令
#[tauri::command]
async fn login_user(id: String) -> Result<LoginResponse, String> {
    let client = Client::new();
    let url = "http://localhost:3000/api/user";  // 或者你的 Vercel API 地址

    // 发送 GET 请求到 Vercel API
    let response = client
        .get(url)
        .query(&[("id", &id)])
        .send()
        .await
        .map_err(|e| format!("登录请求失败: {:?}", e))?;

    // 获取响应文本
    let response_text = response.text().await
        .map_err(|e| format!("读取响应内容失败: {:?}", e))?;

    println!("Login API Response: {}", response_text); // 添加调试日志

    // 尝试解析响应
    match serde_json::from_str::<LoginApiResponse>(&response_text) {
        Ok(api_response) => {
            Ok(LoginResponse {
                success: api_response.success,
                message: api_response.message,  // 可能为 None
                user_data: api_response.data,
            })
        },
        Err(e) => {
            Err(format!("解析响应失败: {} - 响应内容: {}", e, response_text))
        }
    }
}

// 添加用户统计数据结构体
#[derive(Deserialize)]
struct UserStats {
    AI_use_times: i32,
    AI_total_cost: f64,
    Whisper_use_times: i32,
    Whisper_total_cost: f64,
    wallet: f64,
}

// 修改更新用户统计信息的结构体
#[derive(Serialize, Deserialize, Debug)]
struct UpdateStatsRequest {
    AI_use_times: i32,
    AI_input_tokens: i32,
    AI_output_tokens: i32,
    AI_total_cost: f64,
    Whisper_use_times: i32,
    Whisper_total_cost: f64,
    Whisper_total_duration: f64,
    wallet: f64,
}

// 修改更新用户统计信息的命令
// #[tauri::command]
// async fn update_user_stats(user_id: String, stats: UpdateStatsRequest) -> Result<UpdateUserResponse, String> {
//     let client = Client::new();
//     let url = "http://localhost:3000/api/user";

//     println!("Updating stats for user {}: {:?}", user_id, stats);

//     let response = client
//         .put(url)
//         .header("Content-Type", "application/json")
//         .query(&[("id", &user_id)])
//         .json(&stats)
//         .send()
//         .await
//         .map_err(|e| format!("更新请求失败: {:?}", e))?;

//     let response_text = response.text().await
//         .map_err(|e| format!("读取响应内容失败: {:?}", e))?;

//     println!("Update response: {}", response_text);

//     match serde_json::from_str::<UpdateUserResponse>(&response_text) {
//         Ok(response) => Ok(response),
//         Err(e) => Err(format!("解析响应失败: {} - 响应内容: {}", e, response_text))
//     }
// }
// 修改更新用户统计信息的命令
#[tauri::command] 
async fn update_user_stats(user_id: String, AI_use_times: i32, AI_input_tokens: i32, AI_output_tokens: i32, AI_total_cost: f64, Whisper_use_times: i32, Whisper_total_cost: f64, Whisper_total_duration: f64, wallet: f64) -> Result<UpdateUserResponse, String> {
    let client = Client::new();
    //let url = "https://eplayer-server.vercel.app/api/user";
    let url = "http://localhost:3000/api/user";

    // 发送 PUT 请求到 Vercel API
    let response = client
        .put(url)
        .header("Content-Type", "application/json")
        .query(&[
            ("id", &user_id),
            ("AI_use_times", &AI_use_times.to_string()),
            ("AI_input_tokens", &AI_input_tokens.to_string()),
            ("AI_output_tokens", &AI_output_tokens.to_string()),
            ("AI_total_cost", &AI_total_cost.to_string()),
            ("Whisper_use_times", &Whisper_use_times.to_string()),
            ("Whisper_total_cost", &Whisper_total_cost.to_string()),
            ("Whisper_total_duration", &Whisper_total_duration.to_string()),
            ("wallet", &wallet.to_string()),
        ])
        .send()
        .await
        .map_err(|e| format!("更新请求失败: {:?}", e))?;

    // 获取响应文本
    let response_text = response.text().await
        .map_err(|e| format!("读取响应内容失败: {:?}", e))?;

    println!("Update API Response: {}", response_text); // 添加调试日志

    // 尝试解析响应
    match serde_json::from_str::<UpdateUserResponse>(&response_text) {
        Ok(response) => Ok(response),
        Err(e) => Err(format!("解析响应失败: {} - 响应内容: {}", e, response_text))
    }
}


// 添加获取用户数据的响应结构体
#[derive(Deserialize)]
struct UserData {
    success: bool,
    data: Option<UserDataDetails>,
    message: Option<String>,
}

// 添加获取用户数据的命令
#[tauri::command]
async fn get_user_data(user_id: String) -> Result<UserDataDetails, String> {
    let client = Client::new();
    let url = "http://localhost:3000/api/user";

    // 发送 GET 请求到 Vercel API
    let response = client
        .get(url)
        .query(&[("id", &user_id)])
        .send()
        .await
        .map_err(|e| format!("获取用户数据失败: {:?}", e))?;

    let response_text = response.text().await
        .map_err(|e| format!("读取响应内容失败: {:?}", e))?;

    println!("Get User Data Response: {}", response_text);

    // 解析响应
    let user_data: UserData = serde_json::from_str(&response_text)
        .map_err(|e| format!("解析响应失败: {} - 响应内容: {}", e, response_text))?;

    if user_data.success {
        user_data.data.ok_or("用户数据为空".to_string())
    } else {
        Err(user_data.message.unwrap_or("获取用户数据失败".to_string()))
    }
}

#[derive(Serialize, Deserialize)]
struct UserDataDetails {
    email: String,
    native_language: String,
    version: String,
    AI_use_times: i32,
    AI_input_tokens: i32,
    AI_output_tokens: i32,
    AI_total_cost: f64,
    Whisper_use_times: i32,
    Whisper_total_cost: f64,
    Whisper_total_duration: f64,
    wallet: f64,
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
            search_subtitles,
            download_subtitle,
            calculate_md5,
            register_user,
            update_user_version,
            login_user,  // 添加登录命令
            update_user_stats,  // 添加更新用户统计信息的命令
            get_user_data  // 添加新命令
        ])
        .plugin(tauri_plugin_log::Builder::new().build())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
