use kovi::Message as KoviMessage;
use kovi::PluginBuilder as plugin;
use kovi::tokio;
use kovi::tokio::sync::Mutex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::error::Error;
use std::sync::Arc;
use std::env;

// 数据结构定义
#[derive(Debug, Deserialize, Serialize, Clone)]
struct ChatCompletionResponse {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<Choice>,
    usage: Usage,
    system_fingerprint: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ToolCalls {
    id: String,
    r#type: String,
    function: FunctionCalls,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct FunctionCalls {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct SearchKnowledgeBaseArguments {
    query: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Choice {
    index: u32,
    message: Message,
    finish_reason: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Message {
    role: String,
    content: String,
    tool_calls: Option<Vec<ToolCalls>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct MessageWithoutToolCalls {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct SearchResponse {
    messages: Vec<SearchResponseMessage>,
    summary: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct SearchResponseMessage {
    url: String,
    title: String,
    content: String,
    icon: String,
}

// 配置结构体
#[derive(Clone)]
struct DeepSeekConfig {
    api_key: String,
    api_url: String,
    search_api_key: String,
    search_url: String,
}

impl DeepSeekConfig {
    fn new() -> Self {
        Self {
            api_key: env::var("DEEPSEEK_API_KEY").unwrap_or_default(),
            api_url: "https://api.deepseek.com/chat/completions".to_string(),
            search_api_key: env::var("BO_CHA_API_KEY").unwrap_or_default(),
            search_url: "https://api.bochaai.com/v1/ai-search".to_string(),
        }
    }
}

// 聊天历史管理器
#[derive(Clone)]
struct ChatHistoryManager {
    history_messages: Arc<Mutex<Vec<Message>>>,
    prompt_list: Arc<Mutex<Vec<Message>>>,
}

impl ChatHistoryManager {
    fn new() -> Self {
        let system_prompt = Message {
            content: "你正在扮演一个真实的聊天对象,我会将消息以`[name]:msg`的格式发送给你,请鉴别不同人的消息记录的同时,综合群聊上下文回答对方问题。用中文回答,注意只回答内容,以纯字符串形式回复,不要带任何格式。".to_string(),
            role: "system".to_string(),
            tool_calls: None,
        };

        Self {
            history_messages: Arc::new(Mutex::new(vec![])),
            prompt_list: Arc::new(Mutex::new(vec![system_prompt])),
        }
    }

    async fn add_message(&self, message: Message) {
        let mut history = self.history_messages.lock().await;
        history.push(message);
    }

    async fn get_combined_messages(&self, additional_messages: Vec<Message>) -> Vec<MessageWithoutToolCalls> {
        let prompt_list = self.prompt_list.lock().await.clone();
        let history_messages = self.history_messages.lock().await.clone();
        
        let combined: Vec<Message> = prompt_list
            .into_iter()
            .chain(history_messages.into_iter())
            .chain(additional_messages.into_iter())
            .collect();

        combined
            .into_iter()
            .map(|item| MessageWithoutToolCalls {
                content: item.content,
                role: item.role,
            })
            .collect()
    }
}

// 知识库搜索服务
struct KnowledgeBaseSearcher {
    client: Client,
    config: DeepSeekConfig,
}

impl KnowledgeBaseSearcher {
    fn new(client: Client, config: DeepSeekConfig) -> Self {
        Self { client, config }
    }

    async fn search(&self, query: String) -> Result<String, Box<dyn Error + Send + Sync>> {
        let request_body = json!({
            "query": &query,
            "freshness": "noLimit",
            "count": 10,
            "answer": false,
            "stream": false
        });

        let response = self.client
            .post(&self.config.search_url)
            .header("Content-Type", "application/json")
            .header("Accept", "*/*")
            .header("Connection", "keep-alive")
            .header("Authorization", format!("Bearer {}", self.config.search_api_key))
            .json(&request_body)
            .send()
            .await?;

        if response.status().is_success() {
            let response_text = response.text().await?;
            let response_json: SearchResponse = serde_json::from_str(&response_text)?;
            
            let formatted_results = response_json.messages
                .iter()
                .map(|msg| format!("标题: {}\n内容: {}\n来源: {}\n", msg.title, msg.content, msg.url))
                .collect::<Vec<String>>()
                .join("\n---\n");

            Ok(format!("搜索结果:\n{}", formatted_results))
        } else {
            Err(format!("搜索请求失败: {:?}", response.status()).into())
        }
    }
}

// DeepSeek AI 服务
struct DeepSeekService {
    client: Client,
    config: DeepSeekConfig,
    history_manager: ChatHistoryManager,
    knowledge_searcher: KnowledgeBaseSearcher,
}

impl DeepSeekService {
    fn new() -> Self {
        let client = Client::new();
        let config = DeepSeekConfig::new();
        let history_manager = ChatHistoryManager::new();
        let knowledge_searcher = KnowledgeBaseSearcher::new(client.clone(), config.clone());

        Self {
            client,
            config,
            history_manager,
            knowledge_searcher,
        }
    }

    async fn chat(&self, messages: Vec<Message>, enable_tools: bool) -> String {
        let combined_messages = if enable_tools {
            self.history_manager.get_combined_messages(messages.clone()).await
        } else {
            self.history_manager.get_combined_messages(messages.clone()).await
        };

        let request_body = self.build_request_body(&combined_messages, enable_tools);

        match self.send_chat_request(&request_body).await {
            Ok(response_text) => {
                if let Ok(response_json) = serde_json::from_str::<ChatCompletionResponse>(&response_text) {
                    if let Some(choice) = response_json.choices.first() {
                        // 处理工具调用
                        if let Some(tool_calls) = &choice.message.tool_calls {
                            return self.handle_tool_calls(tool_calls, messages).await;
                        }
                        
                        // 添加到历史记录
                        if enable_tools {
                            for msg in messages {
                                self.history_manager.add_message(msg).await;
                            }
                        }
                        
                        choice.message.content.clone()
                    } else {
                        "未收到有效回复".to_string()
                    }
                } else {
                    format!("解析响应失败: {}", response_text)
                }
            }
            Err(e) => format!("请求失败: {:?}", e)
        }
    }

    async fn send_chat_request(&self, request_body: &serde_json::Value) -> Result<String, Box<dyn Error + Send + Sync>> {
        let response = self.client
            .post(&self.config.api_url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(request_body)
            .send()
            .await?;

        let response_text = response.text().await?;
        Ok(response_text)
    }

    fn build_request_body(&self, messages: &[MessageWithoutToolCalls], enable_tools: bool) -> serde_json::Value {
        let mut request = json!({
            "messages": messages,
            "model": "deepseek-chat",
            "frequency_penalty": 0,
            "response_format": { "type": "text" },
            "stop": null,
            "stream": false,
            "stream_options": null,
            "top_p": 1,
            "n": 1,
        });

        if enable_tools {
            request["max_tokens"] = json!(2048);
            request["temperature"] = json!(1.1);
            request["tools"] = json!([{
                "type": "function",
                "function": {
                    "name": "search_knowledge_base",
                    "description": "联网搜索用户提出的相关问题。",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": "用户的问题中需要搜索查询的问题。"
                            }
                        },
                        "required": ["query"]
                    },
                    "strict": true
                }
            }]);
        } else {
            request["max_tokens"] = json!(8000);
            request["temperature"] = json!(0.7);
        }

        request
    }

    async fn handle_tool_calls(&self, tool_calls: &[ToolCalls], original_messages: Vec<Message>) -> String {
        for tool_call in tool_calls {
            if tool_call.function.name == "search_knowledge_base" {
                if let Ok(args) = serde_json::from_str::<SearchKnowledgeBaseArguments>(&tool_call.function.arguments) {
                    match self.knowledge_searcher.search(args.query).await {
                        Ok(search_results) => {
                            let search_message = Message {
                                role: "user".to_string(),
                                content: search_results,
                                tool_calls: None,
                            };
                            
                            return Box::pin(self.chat(vec![search_message], false)).await;
                        }
                        Err(e) => return format!("搜索失败: {:?}", e),
                    }
                }
            }
        }
        "工具调用处理失败".to_string()
    }
}

// 工具函数
fn remove_prefix_if_starts_with(input: &str, prefix: &str) -> Option<String> {
    if input.starts_with(prefix) {
        Some(input[prefix.len()..].to_string())
    } else {
        None
    }
}

#[kovi::plugin]
async fn main() {
    let deepseek_service = Arc::new(DeepSeekService::new());

    plugin::on_msg(move |event| {
        let deepseek_service = deepseek_service.clone();
        async move {
            if let Some(plain_text) = event.borrow_text() {
                // 处理 AI 对话请求
                if let Some(content) = remove_prefix_if_starts_with(&plain_text, "ai ") {
                    let user_message = Message {
                        role: "user".to_string(),
                        content: format!("[{}]: {}", 
                            event.sender.nickname.as_ref().unwrap_or(&"Unknown".to_string()), 
                            content
                        ),
                        tool_calls: None,
                    };

                    let response = deepseek_service.chat(vec![user_message], true).await;
                    event.reply_and_quote(&response);
                }
                // 处理简单对话请求
                else if let Some(content) = remove_prefix_if_starts_with(&plain_text, "chat ") {
                    let user_message = Message {
                        role: "user".to_string(),
                        content: format!("[{}]: {}", 
                            event.sender.nickname.as_ref().unwrap_or(&"Unknown".to_string()), 
                            content
                        ),
                        tool_calls: None,
                    };

                    let response = deepseek_service.chat(vec![user_message], false).await;
                    event.reply_and_quote(&response);
                }
            }
        }
    });
}
