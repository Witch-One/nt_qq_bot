use kovi::Message as KoviMessage;
use kovi::PluginBuilder as plugin;
use kovi::tokio;
use kovi::tokio::sync::Mutex; // 使用 tokio 的 Mutex
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::error::Error;
use std::sync::Arc;

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
    conversation_id: String,
    messages: Vec<SearchResponseMessage>,
    code: i64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct SearchResponseMessage {
    role: String,
    r#type: String,
    content_type: String,
    content: String,
}

fn remove_prefix_if_starts_with(input: &str, prefix: &str) -> Option<String> {
    // 判断 input 是否以 prefix 开头
    if input.starts_with(prefix) {
        // 删除 prefix 后的字符串
        input.strip_prefix(prefix).map(|s| s.to_string())
    } else {
        // 如果不以 prefix 开头，返回 None
        None
    }
}

#[kovi::plugin]
async fn main() {
    // 设置 API 密钥
    let api_key = "sk-kghllfzjpsjfwysmetklfsfnlmzjchewwcnfxbknhtdfqcjj";
    let api_url = "https://api.siliconflow.cn/v1/chat/completions";

    // 创建 HTTP 客户端
    let client = Client::new();

    // 使用 Arc<Mutex<Vec<Message>>> 来共享和同步状态
    let history_messages: Arc<Mutex<Vec<Message>>> = Arc::new(Mutex::new(vec![]));

    let prompt_list: Arc<Mutex<Vec<Message>>> = Arc::new(Mutex::new(vec![Message {
        content: "你正在扮演一个真实的聊天对象,我会将消息以`[name]:msg`的格式发送给你,请鉴别不同人的消息记录的同时,综合群聊上下文回答对方问题。用中文回答,注意只回答内容,以纯字符串形式回复,不要带任何格式。".to_string(),
        role: "system".to_string(),
        tool_calls:None,
    }]));

    async fn search_knowledge_base(
        client: Client,
        api_url: String,
        api_key: String,
        history_messages: Arc<Mutex<Vec<Message>>>,
        prompt_list: Arc<Mutex<Vec<Message>>>,
        query: String,
    ) -> String {
        let serch_url = "https://api.bochaai.com/v1/ai-search";
        let client_clone = client.clone();
        let search_api_key = "sk-a2dc9a18f74746328e8d2cce927a2bec";
        let request_body = json!({
            "query": &query,
            "freshness": "noLimit",
            "count": 10,
            "answer": false,
            "stream": false
        });

        let response = client
            .post(serch_url)
            .header("Content-Type", "application/json")
            .header("Accept", "*/*")
            .header("Connection", "keep-alive ")
            .header("Authorization", format!("Bearer {}", search_api_key))
            .json(&request_body)
            .send()
            .await;

        match response {
            Ok(response) => {
                if response.status().is_success() {
                    let response_text = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Failed to read response".to_string());
                    println!("Response: {}", response_text);
                    let response_json: SearchResponse =
                        serde_json::from_str(&response_text).unwrap();

                    let mut search_list: Vec<Message> = vec![];

                    for msg in response_json.messages.into_iter() {
                        search_list.push(Message {
                            content: msg.content,
                            role: "assistant".to_string(),
                            tool_calls: None,
                        })
                    }

                    println!("search_list :{:?}", search_list);
                    // 使用 Box::pin 包装递归调用
                    let chat_res = Box::pin(chat_with_gpt(
                        client,
                        api_url,
                        api_key,
                        history_messages,
                        prompt_list,
                        search_list,
                        false,
                    ))
                    .await;
                    return chat_res;
                } else {
                    eprintln!("Failed to get a successful response: {:?}", response);
                    return format!("Error: {:?}", response);
                }
            }
            Err(e) => {
                eprintln!("Failed to send request: {:?}", e);
                return format!("Error: {:?}", e);
            }
        }
    }

    async fn chat_with_gpt(
        client: Client,
        api_url: String,
        api_key: String,
        history_messages: Arc<Mutex<Vec<Message>>>,
        prompt_list: Arc<Mutex<Vec<Message>>>,
        list: Vec<Message>,
        should_add_to_history: bool,
    ) -> String {
        // 构建请求体
        let request_body = {
            let mut history_messages = history_messages.lock().await; // 使用 .await 获取锁
            // 克隆历史消息到局部变量
            let history_messages_clone = {
                if should_add_to_history {
                    for msg in list.into_iter() {
                        history_messages.push(msg.clone());
                    }
                    history_messages.clone()
                } else {
                    let mut history_messages_clone = history_messages.clone();
                    for msg in list.into_iter() {
                        history_messages_clone.push(msg.clone());
                    }
                    history_messages_clone
                }
            };

            let prompt_list = prompt_list.lock().await.clone();
            let combined: Vec<Message> = prompt_list
                .into_iter()
                .chain(history_messages_clone.into_iter())
                .collect();
            let combined: Vec<MessageWithoutToolCalls> = combined
                .into_iter()
                .map(|item| MessageWithoutToolCalls {
                    content: item.content.clone(),
                    role: item.role.clone(),
                })
                .collect();
            println!("{:?}", combined);

            if !should_add_to_history {
                json!({
                    "messages": combined,
                    "model": "Pro/deepseek-ai/DeepSeek-R1",
                    "frequency_penalty": 0,
                    "max_tokens": 10000,
                    "response_format": {
                        "type": "text"
                    },
                    "stop": null,
                    "stream": false,
                    "stream_options": null,
                    "temperature": 1.1,
                    "top_p": 1,
                    "n": 1,
                })
            }else {
                json!({
                    "messages": combined,
                    "model": "deepseek-ai/DeepSeek-V3",
                    "frequency_penalty": 0,
                    "max_tokens": 2048,
                    "response_format": {
                        "type": "text"
                    },
                    "stop": null,
                    "stream": false,
                    "stream_options": null,
                    "temperature": 1.1,
                    "top_p": 1,
                    "n": 1,
                    "tools":[{
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
                                    },
                                },
                                "required": [
                                    "query"
                                ]
                            },
                            "strict": true
                        }
                    }],
                })
            }
           
        };
        // 发送 POST 请求

        let response = client
            .post(api_url.clone())
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("Authorization", format!("Bearer {}", &api_key))
            .json(&request_body)
            .send()
            .await;
        match response {
            Ok(response) => {
                if response.status().is_success() {
                    let response_text = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Failed to read response".to_string());
                    println!("Response: {}", response_text);
                    let response_json: ChatCompletionResponse =
                        serde_json::from_str(&response_text).unwrap();
                    let first_msg = response_json.choices[0].message.clone();
                    println!("First Choice{:?}", first_msg);
                    if first_msg.tool_calls.is_some() {
                        let tool_calls = first_msg.tool_calls.unwrap();
                        for tool in tool_calls.into_iter() {
                            println!("{:?}", tool);
                            if tool.function.name == "search_knowledge_base" {
                                let arguments: SearchKnowledgeBaseArguments =
                                    serde_json::from_str(&tool.function.arguments).unwrap();
                                let serach_res = search_knowledge_base(
                                    client,
                                    api_url.clone(),
                                    api_key.clone(),
                                    history_messages,
                                    prompt_list,
                                    arguments.query.to_string(),
                                )
                                .await;
                                println!("serach_res:{}", serach_res);
                                return serach_res;
                            }
                        }
                    };
                    {
                        let mut history_messages = history_messages.lock().await;
                        history_messages.push(response_json.choices[0].message.clone());
                    };

                    return response_json.choices[0].message.content.to_string();
                } else {
                    eprintln!("Failed to get a successful response: {:?}", response);
                    return format!("Error: {:?}", response);
                }
            }
            Err(e) => {
                eprintln!("Failed to send request: {:?}", e);
                return format!("Error: {:?}", e);
            }
        }
    }

    plugin::on_msg(move |event| {
        let client_clone = client.clone();
        let api_url_clone = api_url.to_string();
        let api_key_clone = api_key.to_string();
        let history_messages_clone = Arc::clone(&history_messages);
        let prompt_list_clone = Arc::clone(&prompt_list);

        println!("{:?}", event.message.to_human_string());
        async move {
            if event.raw_message.contains("[CQ:at,qq=3939271104]") {
                let list = vec![Message {
                    content: format!(
                        "[{}]:{}",
                        event
                            .sender
                            .nickname
                            .as_ref()
                            .unwrap_or(&"Unknown".to_string()),
                        event.borrow_text().unwrap().clone()
                    ),
                    role: "user".to_string(),
                    tool_calls: None,
                }];

                let res = chat_with_gpt(
                    client_clone,
                    api_url_clone,
                    api_key_clone,
                    prompt_list_clone,
                    history_messages_clone,
                    list,
                    true,
                )
                .await;
                event.reply(
                    KoviMessage::new()
                        .add_reply(event.message_id)
                        .add_at(&event.sender.user_id.to_string().as_str())
                        .add_text(&res),
                );
                return;
            }
            if event
                .borrow_text()
                .unwrap()
                .to_string()
                .starts_with("/system")
                && event.sender.user_id == 1335515386
            {
                let mut prompt_list = prompt_list_clone.lock().await;
                match remove_prefix_if_starts_with(event.borrow_text().unwrap(), "/system") {
                    Some(prompt) => {
                        prompt_list.push(Message {
                            content: prompt,
                            role: "system".to_string(),
                            tool_calls: None,
                        });
                    }
                    None => {
                        event.reply_and_quote("Error: 请以\"/system\"开头以添加prompt");
                    }
                }
            }
            if event
                .borrow_text()
                .unwrap()
                .to_string()
                .starts_with("/clear")
                && event.sender.user_id == 1335515386
            {
                let mut history_messages = history_messages_clone.lock().await;
                history_messages.clear();
                let mut prompt_list = prompt_list_clone.lock().await;
                prompt_list.truncate(1);
                event.reply_and_quote("重置历史消息成功");
            }
        }
    });
}
