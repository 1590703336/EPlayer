import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import ReactPlayer from 'react-player';
import "./App.css";
import SrtParser2 from "srt-parser-2"; // 导入 srt-parser-2 以处理字幕文件的解析
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { open as openUrl } from '@tauri-apps/plugin-shell';   // 用于打开URL
import { getVersion } from '@tauri-apps/api/app';  // 导入获取版本号的函数
import guideImage1 from './assets/guide1.png';
import api from './api';
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from '@tauri-apps/api/core';
import { warn, debug, trace, info, error } from '@tauri-apps/plugin-log';
import { writeTextFile } from '@tauri-apps/plugin-fs';
import { dirname } from '@tauri-apps/api/path';



function App() {
  
  // 定义各种状态变量来存储视频文件路径、字幕、当前播放时间、字幕索引、播放速度等
  const [videoUrl, setVideoUrl] = useState(""); // 用于存储视频文件路径
  const [subtitles, setSubtitles] = useState([]); // 用于存解析后的字幕
  const [currentTime, setCurrentTime] = useState(0); // 用于存储当前播放时间
  const [currentSubtitleIndex, setCurrentSubtitleIndex] = useState(0); // 用于存储当前字幕索引
  const [playbackRate, setPlaybackRate] = useState(1); // 用于播放速度
  const playerRef = useRef(null); // 使用 useRef 获取 ReactPlayer 的引用，便于直接控制播放器
  const [networkVideoUrl, setNetworkVideoUrl] = useState(""); // 用于存储网络视频链接
  const [isLocalVideo, setIsLocalVideo] = useState(false); // 用于存储是否为本地视频的状态
  const [isNetworkVideo, setIsNetworkVideo] = useState(false); // 用于存储是否为网络视频的状态
  const [isRepeating, setIsRepeating] = useState(false); // 用于存储是否重复播放当前字幕
  // const [updateInfo, setUpdateInfo] = useState(null); // 用于存储更新信息
  // const [isModalOpen, setIsModalOpen] = useState(false); // 用于存储更新弹窗的状态
  const [showAboutMenu, setShowAboutMenu] = useState(false); // 添加状态控制菜单显示
  const [appVersion, setAppVersion] = useState(''); // 获取应用版本号
  const [showGuide, setShowGuide] = useState(false); // 添加状态控制 Guide 显示
  const [selectedWord, setSelectedWord] = useState(""); // 存储选中的词
  const [contextMenu, setContextMenu] = useState({ visible: false, x: 0, y: 0 }); // 控制上下文菜单
  const [showResponse, setShowResponse] = useState(false); // 添加新的状态控制 AI 响应窗口的显示
  const [showCustomInput, setShowCustomInput] = useState(false);  // 控制自输入显
  const [customPrompt, setCustomPrompt] = useState("");  // 存储自定义输入内容
  const [response, setResponse] = useState(""); // 添加状态控制 OpenAI 回复
  const [isPlaying, setIsPlaying] = useState(true); // 添加新的状态来控制播放状态
  const [aiStats, setAiStats] = useState({
    callCount: 0,
    inputTokens: 0,
    outputTokens: 0
  });
  // const [isExtracting, setIsExtracting] = useState(false);
  // const [audioUrl, setAudioUrl] = useState("");
  const [isGeneratingSubtitles, setIsGeneratingSubtitles] = useState(false);
  const [uploadedFile, setUploadedFile] = useState(null);
  const [whisperStats, setWhisperStats] = useState({
    callCount: 0,
    totalDuration: 0
  });
  const [isSearchingSubtitles, setIsSearchingSubtitles] = useState(false);
  const [subtitleSearchResults, setSubtitleSearchResults] = useState([]);
  const [subtitleSearchQuery, setSubtitleSearchQuery] = useState("");
  const [showSearchResults, setShowSearchResults] = useState(true);
  const [whisperLanguage, setWhisperLanguage] = useState("en"); // 默认英语
  const [isSearchInputFocused, setIsSearchInputFocused] = useState(false);
  const [videoMd5, setVideoMd5] = useState("");
  const [showRegister, setShowRegister] = useState(false); // 控制注册弹窗显示
  const [registerForm, setRegisterForm] = useState({
    username: '',
    email: '',
    password: '',
    native_language: 'en' // 默认英
  });
  const [isRegistering, setIsRegistering] = useState(false);
  const [loginForm, setLoginForm] = useState({
    username: '',
    password: ''
  });
  const [isLoggingIn, setIsLoggingIn] = useState(false);
  const [shouldCancelGeneration, setShouldCancelGeneration] = useState(false);
  const [currentUserId, setCurrentUserId] = useState(null);
  const [showNotebook, setShowNotebook] = useState(false);
  const [notebook, setNotebook] = useState([]);
  const [isLoadingNotebook, setIsLoadingNotebook] = useState(false);
  const [isMd5Calculated, setIsMd5Calculated] = useState(false);
  const [token, setToken] = useState(null);
  const [uploadedFilePath, setUploadedFilePath] = useState(null);

  // 获取应用版本号
  useEffect(() => {
    getVersion().then(version => {
      setAppVersion(version);
    });
  }, []);

  // 处理点击其他区域关闭菜单
  useEffect(() => {
    const handleClickOutside = (event) => {
      if (showAboutMenu && !event.target.closest('.about-container')) {
        setShowAboutMenu(false);
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [showAboutMenu]);

  // 处理字幕文件上传
  function handleSubtitleUpload(event) {
    const file = event.target.files[0]; // 获取文件对象
    if (file) {
      const reader = new FileReader(); // 创建 FileReader 实例以取文件内容
      reader.onload = (e) => {
        const content = e.target.result; // 读取的文件内容
        const parser = new SrtParser2(); // 创建 SrtParser2 实例以解析字幕文件
        const parsedSubtitles = parser.fromSrt(content); // 使用 fromSrt 方法解析字幕内容
        setSubtitles(parsedSubtitles); // 更新状态，设置解析后的字幕
      };
      reader.readAsText(file); // 读取文件内容为文本
    }
  }

  useEffect(() => {
    const checkUpdate = async () => {
      const update = await check();
      if (update) {
        console.log(
          `found update ${update.version} from ${update.date} with notes ${update.body}`
        );
        let downloaded = 0;
        let contentLength = 0;
        await update.downloadAndInstall((event) => {
          switch (event.event) {
            case 'Started':
              contentLength = event.data.contentLength;
              console.log(`started downloading ${event.data.contentLength} bytes`);
              break;
            case 'Progress':
              downloaded += event.data.chunkLength;
              console.log(`downloaded ${downloaded} from ${contentLength}`);
              break;
            case 'Finished':
              console.log('download finished');
              break;
          }
        });
        console.log('update installed');
        await relaunch();
      }
    };

    checkUpdate();
  }, []);

  // 修改键盘事件处理函数
  useEffect(() => {
    const handleKeyDown = (event) => {
      if (showRegister || isSearchInputFocused || isGeneratingSubtitles) {
        return;
      }

      if (event.key === ' ') {
        event.preventDefault();
        setIsPlaying(prev => !prev);
      } else if (event.key === 'ArrowLeft') {
        const newIndex = Math.max(currentSubtitleIndex - 1, 1);
        const startTime = subtitles[newIndex - 1]?.startSeconds;
        if (playerRef.current) {
          playerRef.current.seekTo(startTime, 'seconds');
        }
        setCurrentSubtitleIndex(newIndex);
      } else if (event.key === 'ArrowRight') {
        //console.log("ArrowRight");
        //const newIndex = Math.min(currentSubtitleIndex + 1, subtitles.length - 1);
        const newIndex = Math.min(currentSubtitleIndex + 1, subtitles.length);
        const startTime = subtitles[newIndex - 1]?.startSeconds;

        if (playerRef.current) {
          playerRef.current.seekTo(startTime, 'seconds');
        }
        setCurrentSubtitleIndex(newIndex);
      } else if (event.key === 'r') {
        setIsRepeating((prev) => !prev);
      } else if (event.key === 'ArrowUp') {
        setPlaybackRate((prevRate) => Math.min(prevRate + 0.1, 2));
      } else if (event.key === 'ArrowDown') {
        setPlaybackRate((prevRate) => Math.max(prevRate - 0.1, 0.5));
      } else if (event.key.toLowerCase() === 'h') {
        setShowGuide(prev => !prev);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [
    subtitles,
    currentSubtitleIndex,
    isSearchInputFocused,
    showRegister,
    isGeneratingSubtitles  // 添加到依赖数组
  ]);

  // 处理当前播放时间的变化来更新当前的字幕索引
  useEffect(() => {
    if (isRepeating && subtitles.length > 0) {
      const currentSubtitle = subtitles[currentSubtitleIndex - 1]; // 获取当前字幕
      console.log(currentSubtitle);
      if (currentSubtitle) {
        const { startSeconds, endSeconds } = currentSubtitle;
        if (currentTime >= endSeconds) { // 如果当前时间超过了字幕的结束时间
          if (playerRef.current) {
            playerRef.current.seekTo(startSeconds, 'seconds'); // 跳转到当前字幕的开始时间
          }
        }
      }
    } else if (subtitles.length > 0) {
      const currentSubtitle = subtitles.findIndex(subtitle => {
        const startTime = subtitle.startSeconds;
        const endTime = subtitle.endSeconds;
        return currentTime >= startTime && currentTime <= endTime; // 找到当前播放时间对应的字幕
      });
      if (currentSubtitle !== -1) {
        setCurrentSubtitleIndex(Number(subtitles[currentSubtitle].id)); // 更新当前字幕索引         
      }
    }
  }, [currentTime, subtitles]); // 依赖于当前播放时间变化

  // 当字幕信息发生变化时,构建并保存SRT文件
  useEffect(() => {
    const saveSrtFile = async () => {  // 添加 async 函数
      if (subtitles.length > 0 && uploadedFilePath) {
        // 构建SRT内容
        try {
          const srtContent = subtitles.map(subtitle => {
            const startTime = formatSrtTime(subtitle.startSeconds);
            const endTime = formatSrtTime(subtitle.endSeconds);
            return `${subtitle.id}\n${startTime} --> ${endTime}\n${subtitle.text}\n`;
          }).join('\n');

          // 获取视频文件所在目录，并构建srt文件路径
          const srtPath = uploadedFilePath.replace(/\.[^/.]+$/, '.srt');
          console.log("srtPath:", srtPath);

          await invoke("write_file", { path: srtPath, content: srtContent });
          console.log('字幕文件已保存:', srtPath);
          info('字幕文件已保存:', srtPath);
        } catch (error) {
          console.error('保存字幕文件失败:', error);
          error('保存字幕文件失败:', error);
        }
      }
    };

    saveSrtFile();  // 调用这个 async 函数
  }, [subtitles]);

  // 获取当前活跃的字幕文本
  const activeSubtitle = subtitles[currentSubtitleIndex - 1]?.text || '';

  // 处理网络视频链接输入
  const handleNetworkVideoSubmit = async (e) => {
    e.preventDefault(); // 阻止表单默认提交行为
    setVideoUrl(networkVideoUrl); // 设置视频链接
    setIsNetworkVideo(true); // 标记为网络视频
    setIsLocalVideo(false); // 标记不是本地视频

    // 调用获取字幕的函数
    await fetchSubtitles(networkVideoUrl);
  };

  // 获取字幕的函数
  const fetchSubtitles = async (url) => {
    try {
      const subtitles = await invoke("get_transcript", { video: extractVideoId(url) }); // 从后端获取字幕          
      setSubtitles(subtitles); // 设置字幕
      console.log(subtitles);
    } catch (error) {
      console.error("Error fetching subtitles:", error); // 处理获取字幕的错
    }
  };

  // 提取视频ID的数（假设是 YouTube 视频）
  const extractVideoId = (url) => {
    // 处理短链接格式 youtu.be
    if (url.includes('youtu.be')) {
      const pathname = new URL(url).pathname;
      return pathname.slice(1); // 移除开头的斜杠
    }

    // 处理标准格式 youtube.com/watch?v=
    const urlParams = new URLSearchParams(new URL(url).search);
    return urlParams.get('v');
  };

  // 处理本地视频文件上传
  const handleLocalVideoUpload = async () => {
    try {
      const path = await openDialog({
        directory: false,
        multiple: false,
        filters: [{
          name: 'Video',
          extensions: ['mp4', 'webm', 'avi', 'mkv']
        }]
      });
      console.log("path:", path);

      if (path) {
        // 先重置状态
        setIsMd5Calculated(false);
        setVideoMd5("");

        const fileUrl = convertFileSrc(path);
        setUploadedFile(fileUrl);
        setVideoUrl(fileUrl);
        setIsLocalVideo(true);
        setIsNetworkVideo(false);
        setIsPlaying(true);

        try {
          // 计算MD5
          const md5 = await invoke("calculate_md5", { videoPath: path });
          console.log("MD5计算完成:", md5);
          setVideoMd5(md5);
          setIsMd5Calculated(true);
          setUploadedFilePath(path);
        } catch (error) {
          console.error("计算MD5失败:", error);
          // 即使MD5计算失败，也不影响视频播放
          setIsMd5Calculated(false);
          setVideoMd5("");
        }

        // 检查视频文件所在目录是否有同名字幕文件
        const subtitlePath = path.replace(/\.(mp4|webm|avi|mkv)$/, '.srt');
        console.log("subtitlePath:", subtitlePath);
        const vttPath = path.replace(/\.(mp4|webm|avi|mkv)$/, '.vtt');
        await loadSubtitles(subtitlePath);


      }
    } catch (error) {
      console.error("处理文件失败:", error);
      setIsMd5Calculated(false);
      setVideoMd5("");
    }
  };

  // 辅助函数：将文件转换为base64
  // const fileToBase64 = (file) => {
  //   return new Promise((resolve, reject) => {
  //     const reader = new FileReader();
  //     reader.readAsDataURL(file);
  //     reader.onload = () => resolve(reader.result);
  //     reader.onerror = (error) => reject(error);
  //   });
  // };

  // 重置到始状态（主页）
  const resetToHome = () => {
    setShouldCancelGeneration(true); // 设置取消标志
    setVideoUrl(""); // 清空视频链接
    setSubtitles([]); // 清空字幕
    setIsLocalVideo(false); // 重置本地视频标记
    setIsNetworkVideo(false); // 置网络视频标记
    setCurrentTime(0); // 重置当前播放时间
    setCurrentSubtitleIndex(0); // 重置当前字幕索引
    setPlaybackRate(1); // 重置播放速度
    setIsGeneratingSubtitles(false); // 重置生成状态
    setUploadedFile(null); // 清除上传的文件
    setIsMd5Calculated(false); // 重置MD5计算状态
    setUploadedFilePath(null); // 重置上传的文件路径
  };

  // 修改处理"关于"点的函数
  const handleAboutClick = () => {
    setShowAboutMenu(!showAboutMenu);
  };

  // 处理官网点击
  const handleWebsiteClick = async () => {
    await openUrl('https://www.eplayer.fun/');
    setShowAboutMenu(false);
  };

  // 添加处理 Guide 点击的函数
  const handleGuideClick = () => {
    setShowGuide(true);
    setShowAboutMenu(false); // 关闭下拉菜单
  };

  // 添加关闭 Guide 的函数
  const handleCloseGuide = () => {
    setShowGuide(false);
  };

  // 修改处理文本选择的函数
  const handleTextSelection = (event) => {
    const selection = window.getSelection();
    const selectedText = selection.toString().trim();

    if (selectedText) {
      event.preventDefault();

      const range = selection.getRangeAt(0);
      const rect = range.getBoundingClientRect();

      // 检查选中的文本是否包含多词
      const isMultipleWords = selectedText.split(/\s+/).length > 1;

      setSelectedWord(selectedText);
      setContextMenu({
        visible: true,
        x: rect.left,
        y: rect.top - 60,
        isMultipleWords: isMultipleWords // 添加新的属性来标识是否是多个词
      });
    } else {
      setContextMenu({ ...contextMenu, visible: false });
    }
  };

  // 修改更新用户统计信息的函数
  const updateUserStatsToAPI = async (isWhisper, cost, inputTokens = 0, outputTokens = 0, duration = 0) => {
    if (!currentUserId) return;

    try {
      // 获取当前用户统计数据
      const headers = {
        Authorization: `Bearer ${token}`
      };
      const userData = await api.getUser(headers);
      console.log("Current user data:", userData);
      console.log("wallet:", userData.data.data.wallet);

      // 计算新的统计数据
      const newStats = {
        AI_use_times: isWhisper ? userData.data.data.AI_use_times : userData.data.data.AI_use_times + 1,
        AI_input_tokens: userData.data.data.AI_input_tokens + (isWhisper ? 0 : inputTokens),
        AI_output_tokens: userData.data.data.AI_output_tokens + (isWhisper ? 0 : outputTokens),
        AI_total_cost: isWhisper ? userData.data.data.AI_total_cost : userData.data.data.AI_total_cost + cost,
        Whisper_use_times: isWhisper ? userData.data.data.Whisper_use_times + 1 : userData.data.data.Whisper_use_times,
        Whisper_total_cost: isWhisper ? userData.data.data.Whisper_total_cost + cost : userData.data.data.Whisper_total_cost,
        Whisper_total_duration: isWhisper ? userData.data.data.Whisper_total_duration + duration : userData.data.data.Whisper_total_duration,
        wallet: userData.data.data.wallet - cost
      };

      console.log("Updating stats:", newStats);

      // 在后台更新用户数据
      api.updateUser(newStats, headers)
        .then(updateUserResult => {
          if (updateUserResult.data.success) {
            console.log("更新用户数据成功:", updateUserResult.data.data);
          } else {
            console.error("更新用户数据失败:", updateUserResult.data.message);
          }
        })
        .catch(error => {
          console.error("更新用户数据出错:", error);
        });

    } catch (error) {
      console.error("获取用户数据失败:", error);
    }
  };

  // 修改处理菜单项点击的函数
  const handleMenuItemClick = async (role) => {
    // 检查是否登录
    if (!currentUserId || !token) {
      alert('请先登录后再使用查词功能');
      setContextMenu({ ...contextMenu, visible: false });
      setShowRegister(true); // 显示登录窗口
      return;
    }

    try {
      const result = await invoke("communicate_with_openai", {
        prompt: selectedWord,
        role: role
      });

      const cost = parseFloat(calculateCost(result.input_tokens, result.output_tokens));

      // 更新本地统计���示
      setAiStats(prev => ({
        callCount: prev.callCount + 1,
        inputTokens: prev.inputTokens + result.input_tokens,
        outputTokens: prev.outputTokens + result.output_tokens
      }));

      // 立即显示 AI 响应
      setResponse(result.content);
      setShowResponse(true);

      // 在后台异步处理统计信息更新和笔记保存
      (async () => {
        try {
          // 获取当前用户数据
          const headers = {
            Authorization: `Bearer ${token}`
          };
          const userData = await api.getUser(headers);
          const currentNotebook = userData.data.data.notebook || [];

          // 创建新的笔记条目
          const newNote = {
            word: selectedWord,
            role: role,
            response: result.content,
            timestamp: new Date().toISOString()
          };

          // 更新用户数据
          const payload = {
            AI_use_times: userData.data.data.AI_use_times + 1,
            AI_input_tokens: userData.data.data.AI_input_tokens + result.input_tokens,
            AI_output_tokens: userData.data.data.AI_output_tokens + result.output_tokens,
            AI_total_cost: userData.data.data.AI_total_cost + cost,
            wallet: userData.data.data.wallet - cost,
            notebook: [...currentNotebook, newNote]
          };
          const updateUserResult = await api.updateUser(payload, headers);
          if (updateUserResult.data.success) {
            console.log('笔记已保存到用户数据');
          } else {
            console.error('保存笔记失败:', updateUserResult.data.message);
          }
        } catch (error) {
          console.error('保存笔记失败:', error);
        }
      })();

    } catch (error) {
      console.error("Error communicating with OpenAI:", error);
      alert(error.toString());
    }
    setContextMenu({ ...contextMenu, visible: false });
  };

  // 添加处理点击其他区域的函数
  useEffect(() => {
    const handleClickOutside = (event) => {
      // 检查点击是否在响应窗口外
      if (showResponse && !event.target.closest('.ai-response')) {
        setShowResponse(false);
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [showResponse]);

  // 修改处理自定义选项点的逻辑
  const handleCustomClick = () => {
    setShowCustomInput(true);
    setContextMenu({ ...contextMenu, visible: false });
  };

  // 修改处理自定义输入提交的函数
  const handleCustomSubmit = async (e) => {
    e.preventDefault();

    // 检查是否登录
    if (!currentUserId || !token) {
      alert('请先登录后再使用查词功能');
      setShowCustomInput(false);
      setShowRegister(true); // 显示登录窗口
      return;
    }

    try {
      const combinedPrompt = `${selectedWord}的${customPrompt}`;
      const result = await invoke("communicate_with_openai", {
        prompt: combinedPrompt,
        role: "Word_Custom"
      });

      // 立即更新本地显示
      setAiStats(prev => ({
        callCount: prev.callCount + 1,
        inputTokens: prev.inputTokens + result.input_tokens,
        outputTokens: prev.outputTokens + result.output_tokens
      }));

      // 立即显示响应
      setResponse(result.content);
      setShowResponse(true);
      setShowCustomInput(false);
      setCustomPrompt("");

      // 在后台异步处理统计信息更新和笔记保存
      (async () => {
        try {
          const headers = {
            Authorization: `Bearer ${token}`
          };
          console.log("headers:", headers);
          const userData = await api.getUser(headers);
          const currentNotebook = userData.data.data.notebook || [];
          const cost = parseFloat(calculateCost(result.input_tokens, result.output_tokens));

          // 创建新的笔记条目
          const newNote = {
            word: selectedWord,
            customPrompt: customPrompt,
            role: "Word_Custom",
            response: result.content,
            timestamp: new Date().toISOString()
          };

          // 更新用户数据
          const payload = {
            AI_use_times: userData.data.data.AI_use_times + 1,
            AI_input_tokens: userData.data.data.AI_input_tokens + result.input_tokens,
            AI_output_tokens: userData.data.data.AI_output_tokens + result.output_tokens,
            AI_total_cost: userData.data.data.AI_total_cost + cost,
            wallet: userData.data.data.wallet - cost,
            notebook: [...currentNotebook, newNote]
          };
          const updateUserResult = await api.updateUser(payload, headers);
          console.log("updateUserResult:", updateUserResult);
          if (updateUserResult.data.success) {
            console.log('自定义查询笔记已保存');
          } else {
            console.error('保存笔记失败:', updateUserResult.data.message);
          }

        } catch (error) {
          console.error('保存笔记失败:', error);
        }
      })();

    } catch (error) {
      console.error("Error communicating with OpenAI:", error);
      alert(error.toString());
    }
  };

  const loadSubtitles = async (path) => {
    const content = await fetch(convertFileSrc(path)).then(res => res.text());
    const parser = new SrtParser2();
    const parsedSubtitles = parser.fromSrt(content);
    setSubtitles(parsedSubtitles);
  };

  // 在 App 组件中添加计算费用的函数
  function calculateCost(inputTokens, outputTokens) {
    const inputCost = (inputTokens / 1000) * 0.00015;
    const outputCost = (outputTokens / 1000) * 0.0006;
    return (inputCost + outputCost).toFixed(6); // 保留4位小数
  }

  function calculateTotalDuration(totalDuration) {
    const totalMinutes = totalDuration / 60;  //分钟0.006元
    return totalMinutes.toFixed(2);
  }

  // 修改生成 AI 字幕的函数
  const generateAISubtitles = async () => {
    // 检查是否登录
    if (!currentUserId || !token) {
      alert('请先登录后再使用AI字幕功能');
      setShowRegister(true); // 显示登录窗口
      return;
    }

    if (!uploadedFilePath) {
      console.error('没有上传文件');
      alert('请先选择视频文件');
      return;
    }

    //等待视频MD5计算完成
    if (videoMd5 === "") {
      console.error('视频MD5未计算');
      alert('视频MD5正在计算中，请稍后再试');
      return;
    }

    setIsGeneratingSubtitles(true); // 设置生成状态
    setShouldCancelGeneration(false); // 设置取消标志

    try {
      // 首先检查数据库中是否存在字幕
      let subtitleExists = false;

      try {
        const headers = {
          Authorization: `Bearer ${token}`
        };
        console.log("headers:", headers);
        const payload = {
          md5: videoMd5
        };
        const subtitleData = await api.getSubtitle(payload, headers);

        console.log("subtitleData:", subtitleData);
        if (subtitleData.data.success) {
          subtitleExists = true;
          const { user_id, subtitle, play_users_count, play_times, users } = subtitleData.data.data;
          // 解析并设置字幕
          setSubtitles(subtitle);

          if (user_id === currentUserId || users.includes(currentUserId)) {
            // 如果是当前用户的字幕，直接使用，不统计费用
            console.log('使用已有字幕，无需付费');
            const payload = {
              md5: videoMd5,
              play_times: play_times + 1
            };
            await api.updateSubtitle(payload, headers);
            console.log('字幕播放次数已更新');
            return;
          } else {
            // 如果是其他用户的字幕，更新播放次数并统计费用
            const duration = subtitleData.data.data.video_duration;
            const cost = parseFloat((calculateTotalDuration(duration) * 0.006).toFixed(6));

            // 更新本地显示的统计信息
            setWhisperStats(prev => ({
              callCount: prev.callCount + 1,
              totalDuration: prev.totalDuration + duration
            }));

            // 更新用户统计信息
            await updateUserStatsToAPI(true, cost, 0, 0, duration);

            // 更新字幕的播放次数
            const payload = {
              md5: videoMd5,
              play_users_count: play_users_count + 1,
              play_times: play_times + 1,
              users: [...users, currentUserId] // 添加当前用户ID到用户列表
            };
            await api.updateSubtitle(payload, headers);
            console.log('字幕播放次数已更新');
            console.log(`使用其他用户字幕，计费 $${cost}，时长 ${calculateTotalDuration(duration)} 分钟`);
            return;
          }
        }
      } catch (error) {
        if (error.response && error.response.status === 404) {
          console.log('字幕不存在，开始生成新字幕...');
          subtitleExists = false;
        } else {
          throw error;
        }
      }

      // 如果字幕不存在，则生成新的字幕
      if (!subtitleExists) {
        console.log('开始提取音频...');
        if (shouldCancelGeneration) {
          console.log('字幕生成已取消');
          return;
        }

        console.log('开始转写音频...');
        const result = await invoke('transcribe_audio', {
          videoPath: uploadedFilePath,
          language: whisperLanguage,
          jwt: token
        });

        console.log('音频转写完成:', result);
        info('音频转写完成:', result);

        if (!shouldCancelGeneration) {
          const newDuration = result.duration;
          const newCost = parseFloat((calculateTotalDuration(newDuration) * 0.006).toFixed(6));

          // 立即更新本地显示
          setWhisperStats(prev => ({
            callCount: prev.callCount + 1,
            totalDuration: prev.totalDuration + newDuration
          }));

          // 立即设置字幕显示
          setSubtitles(result.subtitles);

          // 在后台异步处理统计信息更新和字幕上传
          (async () => {
            try {

              // 更新用户统计信息
              await updateUserStatsToAPI(true, newCost, 0, 0, newDuration);

              // 保存字幕到数据库
              const headers = {
                Authorization: `Bearer ${token}`
              };
              await api.createSubtitle({
                md5: videoMd5,
                video_duration: newDuration,
                user_id: currentUserId,
                subtitle: result.subtitles,
                play_users_count: 1
              }, headers);

              console.log('字幕已保存到数据库');
              info('字幕已保存到数据库');
            } catch (error) {
              console.error('保存字��信息失败:', error);
              error('保存字幕信息失败:', error)
              // 这里可以添加一些错误提示，但不影响用户继续使用已生成的字幕
            }
          })();
        }
      }
    } catch (error) {
      if (!shouldCancelGeneration) {
        console.error('生成字幕失败:', error);
        alert('生成字幕失败: ' + error);
        error('生成字幕失败: ' + error);
      }
    } finally {
      setIsGeneratingSubtitles(false);
    }
  };

  // 添加格式化SRT时间的辅助函数
  const formatSrtTime = (seconds) => {
    const pad = (num) => num.toString().padStart(2, '0');
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = Math.floor(seconds % 60);
    const ms = Math.floor((seconds % 1) * 1000);
    return `${pad(hours)}:${pad(minutes)}:${pad(secs)},${ms.toString().padStart(3, '0')}`;
  };

  // 添加处理搜索的函数
  const handleSearchSubtitles = async (e) => {
    e.preventDefault();
    if (!subtitleSearchQuery.trim()) {
      alert('请输入要搜索的字幕名称');
      return;
    }

    setIsSearchingSubtitles(true);
    setShowSearchResults(true);  // 显示搜索结果
    try {
      const results = await invoke('search_subtitles', {
        fileName: subtitleSearchQuery
      });
      setSubtitleSearchResults(results);
    } catch (error) {
      console.error('搜索字幕失败:', error);
      alert('搜索字幕失败: ' + error);
    } finally {
      setIsSearchingSubtitles(false);
    }
  };

  // 修改处理下载字幕的函数
  const handleDownloadSubtitle = async (subtitle) => {
    try {
      console.log('开始下载字幕:', subtitle);
      const content = await invoke('download_subtitle', {
        fileId: subtitle.file_id
      });

      // 将字幕内容解析为字幕对象数组
      const parser = new SrtParser2();
      const parsedSubtitles = parser.fromSrt(content);

      // 更新字幕状态
      setSubtitles(parsedSubtitles);

      // 提示用户字幕已加载
      console.log('字幕已加载');

    } catch (error) {
      console.error('下载字幕失败:', error);
      alert('下载字幕失败: ' + error);
    }
  };

  // 添加时间格式化函数
  const formatTime = (seconds) => {
    const minutes = Math.floor(seconds / 60);
    const remainingSeconds = Math.floor(seconds % 60);
    return `${minutes.toString().padStart(2, '0')}:${remainingSeconds.toString().padStart(2, '0')}`;
  };

  // 在 App 组件中添加处理注册的函数
  const handleRegister = async (e) => {
    e.preventDefault();
    setIsRegistering(true);
    try {
      const payload = {
        username: registerForm.username,
        password: registerForm.password,
        email: registerForm.email,
        native_language: registerForm.native_language
      };
      console.log("registerForm:", payload);
      const createUserResult = await api.createUser(payload);

      if (createUserResult.data.success) {
        // 注册成功后不关闭窗口，而是提示用户登录
        alert("注册成功，请登录");

        // 清空注册表单
        setRegisterForm({
          username: '',
          email: '',
          password: '',
          native_language: 'en'
        });

        // 自动填充登录表单的用户名
        setLoginForm(prev => ({
          ...prev,
          username: payload.username
        }));

      } else {
        alert(createUserResult.data.message);
      }
    } catch (error) {
      console.error("注册失败:", error);
      alert("注册失败: " + error);
    } finally {
      setIsRegistering(false);
    }
  };

  // 修改处理登录的函数
  const handleLogin = async (e) => {
    e.preventDefault();
    setIsLoggingIn(true);
    try {
      console.log("loginForm:", loginForm);

      const result = await api.loginUser({
        username: loginForm.username,
        password: loginForm.password
      });

      //console.log("loginResult:", result);

      if (result.data.success) {
        // 保存token
        setToken(result.data.token);
        // 保存用户ID
        setCurrentUserId(result.data.user.id);  // 注意这里改为 result.data.user.id

        console.log("currentUserId:", result.data.user.id);
        console.log("token:", result.data.token);

        // 使用立即执行的异步函数来处理后续操作
        (async () => {
          try {
            // 更新用户版本信息
            const headers = {
              Authorization: `Bearer ${result.data.token}`
            };
            const payload = {
              version: appVersion,
              last_login: new Date().toISOString()
            };
            const updateUserVersionResult = await api.updateUserVersion(payload, headers);

            console.log("updateUserVersionResult:", updateUserVersionResult);

            if (!updateUserVersionResult.data.success) {
              console.error("更新用户版本失败:", updateUserVersionResult.data.message);
            } else {
              console.log("更新用户版本成功,version:", updateUserVersionResult.data.data.version);
              console.log("更新最后登录时间成功,last_login:", updateUserVersionResult.data.data.last_login);
            }

            // 预加载笔记数据
            //const userData = await api.getUser(result.data.user.id);
            const getNotebookResult = await api.getNotebook(headers);
            console.log("getNotebookResult:", getNotebookResult);
            if (getNotebookResult.data.success) {
              //const userNotebook = getNotebookResult.data.data.notebook || [];
              const userNotebook = getNotebookResult.data.data || [];
              // 按时间倒序排序
              setNotebook(userNotebook.sort((a, b) =>
                new Date(b.timestamp) - new Date(a.timestamp)
              ));
              console.log("笔记数据预加载成功");
            }
          } catch (error) {
            console.error('后台操作失败:', error);
          }
        })();

        // 关闭注册窗口
        setShowRegister(false);
        console.log("登录成功");
      } else {
        alert(result.data.message || "登录失败");
      }
    } catch (error) {
      console.error("登录失败:", error);
      if (error.response) {
        alert(error.response.data.message || "登录失败");
      } else {
        alert("登录失败: " + error.message);
      }
    } finally {
      setIsLoggingIn(false);
    }
  };

  // 修改处理笔记本点击的函数
  const handleNotebookClick = () => {
    // 检查是否登录
    if (!currentUserId || !token) {
      alert('请先登录后再查看笔记本');
      setShowRegister(true); // 显示登录窗口
      return;
    }

    setShowNotebook(true);
    setShowAboutMenu(false);
  };

  // 添加刷笔记的函数
  const refreshNotebook = async () => {
    // 检查是否登录
    if (!currentUserId || !token) {
      alert('请先登录后再查看笔记本');
      setShowNotebook(false); // 关闭笔记本窗口
      setShowRegister(true); // 显示登录窗口
      return;
    }

    setIsLoadingNotebook(true);
    try {
      const headers = {
        Authorization: `Bearer ${token}`
      };
      const getNotebookResult = await api.getNotebook(headers);
      if (getNotebookResult.data.success) {
        const userNotebook = getNotebookResult.data.data || [];
        // 按时间倒序排序
        setNotebook(userNotebook.sort((a, b) =>
          new Date(b.timestamp) - new Date(a.timestamp)
        ));
        console.log("笔记刷新成功");
      }
    } catch (error) {
      console.error('刷新笔记失败:', error);
      alert('刷新笔记失败: ' + error.toString());
    } finally {
      setIsLoadingNotebook(false);
    }
  };

  useEffect(() => {
    if (videoMd5) {
      setIsMd5Calculated(true);
      console.log("MD5已更新，设置计算完成状态");
    }
  }, [videoMd5]);

  const [videoSrc, setVideoSrc] = useState(null);

  const handleDragOver = (event) => {
    console.log("handleDragOver");
    event.preventDefault(); // 阻止默认行为
  };

  const handleDrop = (event) => {
    console.log("handleDrop");
    event.preventDefault();

    // 获取拖拽的文件
    const files = event.dataTransfer.files;
    console.log("files:", files);
    if (files.length > 0) {
      const file = files[0];

      // 检查文件类型是否为视频
      if (file.type.startsWith("video/")) {
        const fileUrl = URL.createObjectURL(file); // 创建临时的文件URL
        setVideoSrc(fileUrl);
      } else {
        alert("请拖拽一个视频文件");
      }
    }
  };

  return (
    <main className="container">
      {/* <div
        onDragOver={handleDragOver}
        onDrop={handleDrop}
        style={{
          width: "100%",
          height: "100%",
          border: "2px dashed gray",
          display: "flex",
          justifyContent: "center",
          alignItems: "center",
        }}
      >
        {videoSrc ? (
          <video controls style={{ width: "80%", height: "80%" }}>
            <source src={videoSrc} type="video/mp4" />
            您的浏览器不支持视频播放
          </video>
        ) : (
          <p>拖拽一个视频文件到此处1</p>
        )}
      </div> */}
      <div className="home-icon" onClick={resetToHome}>
        <i className="fas fa-home"></i>
      </div>
      {/* 修改 about menu 部分 */}
      <div className="about-container">
        <div className="about-icon" onClick={handleAboutClick}>
          <i className="fas fa-info-circle"></i>
        </div>
        {showAboutMenu && (
          <div className="about-menu">
            <div className="menu-item">Version {appVersion}</div>
            {isLocalVideo && videoMd5 && (
              <div className="menu-item">
                Video MD5: {videoMd5}
              </div>
            )}
            <div className="menu-item">
              AI Stats:<br />
              Use Times: {aiStats.callCount}<br />
              Input Tokens: {aiStats.inputTokens}<br />
              Output Tokens: {aiStats.outputTokens}<br />
              Total Cost: ${calculateCost(aiStats.inputTokens, aiStats.outputTokens)}<br />
              <br />
              Whisper Stats:<br />
              Use Times: {whisperStats.callCount}<br />
              Total Duration: {calculateTotalDuration(whisperStats.totalDuration)} minutes
              <br />
              Total Cost: ${calculateTotalDuration(whisperStats.totalDuration) * 0.006}
            </div>
            <div className="menu-item" onClick={handleWebsiteClick}>Visit Website</div>
            <div className="menu-item" onClick={handleGuideClick}>User Guide</div>
            <div className="menu-item" onClick={handleNotebookClick}>
              <i className="fas fa-book"></i> Notebook
            </div>
            <div className="menu-item" onClick={() => currentUserId ? null : setShowRegister(true)}>
              <i className="fas fa-user"></i>
              {currentUserId ? (
                <>已登录: {loginForm.username}</>
              ) : (
                <>注册/登录</>
              )}
            </div>
          </div>
        )}
      </div>

      {/* 添加 Guide 界面 */}
      {showGuide && (
        <div className="guide-overlay">
          <div className="guide-content">
            <div className="guide-header">
              <button className="close-button" onClick={handleCloseGuide}>
                <i className="fas fa-times"></i>
              </button>
            </div>
            <div className="guide-body">
              <div className="guide-section">
                <img src={guideImage1} alt="Guide" className="guide-image" />
              </div>
            </div>
          </div>
        </div>
      )}

      
      <div className="main-content">
        <div className="player-wrapper">
          <ReactPlayer
            ref={playerRef} // 绑定 playerRef 以控制播放器
            url={videoUrl} // 使用 videoUrl 状态
            width="100%"
            height="100%"
            controls={true} // 显示播放控制按钮
            onProgress={({ playedSeconds }) => setCurrentTime(playedSeconds)} // 更新当前播放时间
            playing={isPlaying}  // 使用 isPlaying 状态控制播放
            progressInterval={100} // 每 100 毫秒更新播放进度
            playbackRate={playbackRate} // 设置播放速度
          />
          {/* {audioUrl && (
            <div className="audio-player">
              <audio controls src={audioUrl}>
                您的浏览器不支持音频播放器
              </audio>
            </div>
          )} */}
          <div className="subtitle-overlay">
            <p
              style={{ display: 'flex', justifyContent: 'center', position: 'relative' }}
              onMouseUp={handleTextSelection}
              onContextMenu={(e) => e.preventDefault()}
            >
              <span style={{ position: 'absolute', left: '50%', transform: 'translateX(-50%)' }}>
                {activeSubtitle}
              </span>
              <span style={{ marginLeft: '3px', marginRight: '3px', position: 'absolute', right: '0' }}>
                {playbackRate.toFixed(1)}
              </span>
            </p>
          </div>
          {isLocalVideo && !subtitles.length && (
            <div className="subtitle-buttons">
              <div className="language-select">
                <select
                  value={whisperLanguage}
                  onChange={(e) => setWhisperLanguage(e.target.value)}
                  className="language-dropdown"
                >
                  <option value="en">English</option>
                  <option value="zh">中文</option>
                  <option value="ja">日本語</option>
                  <option value="ko">한국어</option>
                  <option value="fr">Français</option>
                  <option value="de">Deutsch</option>
                  <option value="es">Español</option>
                </select>
              </div>
              <button
                className="ai-subtitle-button"
                onClick={generateAISubtitles}
                disabled={isGeneratingSubtitles}
              >
                {isGeneratingSubtitles ? (
                  <>
                    <div className="loading-spinner" />
                    生成字幕中...
                  </>
                ) : (
                  <>
                    <i className="fas fa-closed-captioning" />
                    生成 AI 字
                  </>
                )}
              </button>
            </div>
          )}
          {showSearchResults && subtitleSearchResults.length > 0 ? (
            <div className="subtitle-search-results">
              <div className="search-results-header">
                <h3>找到的字幕:</h3>
                <button
                  className="close-search-results"
                  onClick={() => setShowSearchResults(false)}
                >
                  <i className="fas fa-times" />
                </button>
              </div>
              <div className="results-list">
                {subtitleSearchResults.map((result, index) => (
                  <div key={index} className="result-item">
                    <span>{result.file_name} ({result.language})</span>
                    <button onClick={() => handleDownloadSubtitle(result)}>
                      <i className="fas fa-download" />
                      下
                    </button>
                  </div>
                ))}
              </div>
            </div>
          ) : (
            showSearchResults && isSearchingSubtitles && (
              <div className="subtitle-search-results">
                <div className="search-results-header">
                  <h3>搜索结果</h3>
                  <button
                    className="close-search-results"
                    onClick={() => setShowSearchResults(false)}
                  >
                    <i className="fas fa-times" />
                  </button>
                </div>
                <p>未找到相关字幕</p>
                <p>建议：</p>
                <ul>
                  <li>尝试使用更简短的关键词</li>
                  <li>检查拼写是否正确</li>
                  <li>尝试用影片的英文名称</li>
                </ul>
              </div>
            )
          )}
        </div>

        <div className="subtitle-search-container">
          <form onSubmit={handleSearchSubtitles}>
            <input
              type="text"
              value={subtitleSearchQuery}
              onChange={(e) => setSubtitleSearchQuery(e.target.value)}
              placeholder="输入字幕名称搜索"
              className="subtitle-search-input"
              onFocus={() => setIsSearchInputFocused(true)}
              onBlur={() => setIsSearchInputFocused(false)}
            />
            <button
              type="submit"
              className="search-subtitle-button"
              disabled={isSearchingSubtitles}
            >
              {isSearchingSubtitles ? (
                <>
                  <div className="loading-spinner" />
                  搜索中...
                </>
              ) : (
                <>
                  <i className="fas fa-search" />
                  搜索字幕
                </>
              )}
            </button>
          </form>
        </div>
        <div style={{ display: 'flex', justifyContent: 'left' }}>
          <div>
            {/* 当既不是本地视频也不是网络视频时，显示视频输入选项 */}
            {!isLocalVideo && !isNetworkVideo && (
              <>
                <form onSubmit={handleNetworkVideoSubmit}>
                  <input
                    type="text"
                    value={networkVideoUrl}
                    onChange={(e) => setNetworkVideoUrl(e.target.value)}
                    placeholder="Enter YouTube link" // 提示用户输入网络视频链接
                  />
                  <button type="submit">Load</button>   {/* 注释加载网络视 */}
                </form>

                <div className="file-input-wrapper">
                  <button
                    onClick={handleLocalVideoUpload}
                  // disabled={isExtracting}
                  >
                    选择本地视频文件
                  </button>
                  {/* {isExtracting && <span className="extracting-status">正在提取音频...</span>} */}
                </div>
              </>
            )}
          </div>
          {/* 如果是本地视频，显示字幕文件输入选项 */}
          {isLocalVideo && (
            <div className="file-input-wrapper">
              <button
                onClick={async () => {
                  try {
                    const path = await openDialog({
                      directory: false,
                      multiple: false,
                      filters: [{
                        name: 'Subtitle',
                        extensions: ['srt', 'vtt']
                      }]
                    });

                    if (path) {
                      // 读取字幕文件内容
                      // const content = await invoke('read_file', { path });

                      await loadSubtitles(path);
                    }
                  } catch (error) {
                    console.error("处理字幕文件失败:", error);
                  }
                }}
              >
                选择字幕文件
              </button>
            </div>
          )}
        </div>
      </div>

      {/* 显示字幕列表使当前字幕自动滚动到可视范围内 */}
      <div className="subtitles" style={{ scrollPaddingTop: 'calc(3 * 1.5em)' }}>
        {subtitles.map((subtitle, index) => {
          const isActive = currentSubtitleIndex == Number(subtitle.id);
          return (
            <div
              key={index}
              className={isActive ? (isRepeating ? 'active-subtitle-repeat' : 'active-subtitle') : ''}
              ref={isActive ? (el) => el && el.scrollIntoView({ behavior: 'smooth', block: 'start' }) : null}
              style={{ marginTop: index === 0 ? 'calc(3 * 1.5em)' : '0' }}
            >
              <p>{subtitle.id}  {formatTime(subtitle.startSeconds)} - {subtitle.text}</p>
            </div>
          );
        })}
      </div>

      {/* 添加上下文菜单 */}
      {contextMenu.visible && (
        <div
          className="context-menu"
          style={{
            position: 'fixed',
            left: `${contextMenu.x}px`,
            top: `${contextMenu.y}px`
          }}
        >
          {contextMenu.isMultipleWords ? (
            // 多词菜单选项
            <>
              <div className="menu-item" onClick={() => handleMenuItemClick("Sentence_Translation")}>
                <i className="fas fa-language"></i>
                翻译
              </div>
              <div className="menu-item" onClick={() => handleMenuItemClick("Sentence_Structure")}>
                <i className="fas fa-project-diagram"></i>
                结构拆分
              </div>
              <div className="menu-item" onClick={() => handleMenuItemClick("Sentence_Copy")}>
                <i className="fas fa-copy"></i>
                仿写
              </div>
              <div className="menu-item" onClick={() => handleMenuItemClick("Sentence_Example")}>
                <i className="fas fa-quote-right"></i>
                例句
              </div>
              <div className="menu-item" onClick={() => handleMenuItemClick("Sentence_Transform")}>
                <i className="fas fa-random"></i>
                转换
              </div>
              <div className="menu-item" onClick={handleCustomClick}>
                <i className="fas fa-cog"></i>
                自定义
              </div>
            </>
          ) : (
            // 单词菜单选项
            <>
              <div className="menu-item" onClick={() => handleMenuItemClick("Word_Dictionary")}>
                <i className="fas fa-book"></i>
                词典
              </div>
              <div className="menu-item" onClick={() => handleMenuItemClick("Word_Symbols")}>
                <i className="fas fa-music"></i>
                音标
              </div>
              <div className="menu-item" onClick={() => handleMenuItemClick("Word_More")}>
                <i className="fas fa-ellipsis-h"></i>
                更多
              </div>
              <div className="menu-item" onClick={() => handleMenuItemClick("Word_Etymology")}>
                <i className="fas fa-history"></i>
                词源
              </div>
              <div className="menu-item" onClick={() => handleMenuItemClick("Word_Example")}>
                <i className="fas fa-quote-right"></i>
                例句
              </div>
              <div className="menu-item" onClick={handleCustomClick}>
                <i className="fas fa-cog"></i>
                自定义
              </div>
            </>
          )}
        </div>
      )}

      {/* AI响应显示区域 */}
      {showResponse && response && (
        <div className="ai-response">
          {response}
        </div>
      )}

      {/* 添加自义输入框 */}
      {showCustomInput && (
        <div className="custom-input-overlay">
          <div className="custom-input-container">
            <form onSubmit={handleCustomSubmit}>
              <input
                type="text"
                value={customPrompt}
                onChange={(e) => setCustomPrompt(e.target.value)}
                placeholder={`关于 "${selectedWord}" 的自定义指令...`}
                autoFocus
              />
              <div className="custom-input-buttons">
                <button type="submit">确定</button>
                <button type="button" onClick={() => setShowCustomInput(false)}>取消</button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* 注册弹窗 */}
      {showRegister && (
        <div className="register-overlay">
          <div className="register-content">
            <div className="register-header">
              <h2>注册账号</h2>
              <button
                className="close-button"
                onClick={() => setShowRegister(false)}
              >
                <i className="fas fa-times"></i>
              </button>
            </div>
            <form onSubmit={handleRegister}>
              <div className="form-group">
                <label htmlFor="username">用户名</label>
                <input
                  type="text"
                  id="username"
                  value={registerForm.username}
                  onChange={(e) => setRegisterForm({
                    ...registerForm,
                    username: e.target.value
                  })}
                  required
                />
              </div>
              <div className="form-group">
                <label htmlFor="email">邮箱</label>
                <input
                  type="email"
                  id="email"
                  value={registerForm.email}
                  onChange={(e) => setRegisterForm({
                    ...registerForm,
                    email: e.target.value
                  })}
                  required
                />
              </div>
              <div className="form-group">
                <label htmlFor="password">密码</label>
                <input
                  type="password"
                  id="password"
                  value={registerForm.password}
                  onChange={(e) => setRegisterForm({
                    ...registerForm,
                    password: e.target.value
                  })}
                  required
                />
              </div>
              <div className="form-group">
                <label htmlFor="native_language">母语</label>
                <select
                  id="native_language"
                  value={registerForm.native_language}
                  onChange={(e) => setRegisterForm({
                    ...registerForm,
                    native_language: e.target.value
                  })}
                >
                  <option value="en">English</option>
                  <option value="zh">中文</option>
                  <option value="ja">日本語</option>
                  <option value="ko">한국어</option>
                  <option value="fr">Français</option>
                  <option value="de">Deutsch</option>
                  <option value="es">Español</option>
                </select>
              </div>
              <button type="submit" disabled={isRegistering}>
                {isRegistering ? (
                  <>
                    <div className="loading-spinner" />
                    注册中...
                  </>
                ) : (
                  "注册"
                )}
              </button>
            </form>

            <div className="divider"></div>

            <div className="login-section">
              <h2>登录账号</h2>
              <form onSubmit={handleLogin}>
                <div className="form-group">
                  <label htmlFor="loginUsername">用户名</label>
                  <input
                    type="text"
                    id="loginUsername"
                    value={loginForm.username}
                    onChange={(e) => setLoginForm({
                      ...loginForm,
                      username: e.target.value
                    })}
                    required
                  />
                </div>
                <div className="form-group">
                  <label htmlFor="loginPassword">密码</label>
                  <input
                    type="password"
                    id="loginPassword"
                    value={loginForm.password}
                    onChange={(e) => setLoginForm({
                      ...loginForm,
                      password: e.target.value
                    })}
                    required
                  />
                </div>
                <button type="submit" disabled={isLoggingIn}>
                  {isLoggingIn ? (
                    <>
                      <div className="loading-spinner" />
                      登录中...
                    </>
                  ) : (
                    "登录"
                  )}
                </button>
              </form>
            </div>
          </div>
        </div>
      )}

      {/* 添加笔记本显示界面 */}
      {showNotebook && (
        <div className="notebook-overlay">
          <div className="notebook-content">
            <div className="notebook-header">
              <h2>学习笔记</h2>
              <div className="notebook-controls">
                <button className="refresh-button" onClick={refreshNotebook}>
                  <i className="fas fa-sync-alt"></i>
                </button>
                <button className="close-button" onClick={() => setShowNotebook(false)}>
                  <i className="fas fa-times"></i>
                </button>
              </div>
            </div>
            <div className="notebook-body">
              {isLoadingNotebook ? (
                <div className="notebook-loading">
                  <div className="loading-spinner"></div>
                  <span>加载中...</span>
                </div>
              ) : notebook.length > 0 ? (
                notebook.map((note, index) => (
                  <div key={index} className="note-item">
                    <div className="note-header">
                      <span className="note-word">{note.word}</span>
                      <span className="note-type">{note.role}</span>
                      <span className="note-time">
                        {new Date(note.timestamp).toLocaleString()}
                      </span>
                    </div>
                    {note.customPrompt && (
                      <div className="note-prompt">
                        自定义提示: {note.customPrompt}
                      </div>
                    )}
                    <div className="note-response">{note.response}</div>
                  </div>
                ))
              ) : (
                <div className="notebook-empty">
                  <p>暂无笔记</p>
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </main>
  );
}

export default App;
