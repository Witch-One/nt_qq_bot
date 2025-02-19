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
struct Choice {
    index: u32,
    message: Message,
    finish_reason: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
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
    }]));

    // async  fn search_knowledge_base(query: String) -> null {

    //     let serch_url = "https://api.bochaai.com/v1/ai-search";
    //     let client_clone = client.clone();

    //     let request_body = json!({
    //         "query": &query,
    //         "freshness": "noLimit",
    //         "count": 10,
    //         "answer": false,
    //         "stream": false
    //     });

    //     let response = client_clone
    //     .post(serch_url)
    //     .header("Content-Type", "application/json")
    //     .header("Accept", "application/json")
    //     .header("Authorization", format!("Bearer {}", api_key_clone))
    //     .json(&request_body)
    //     .send()
    //     .await;
    // }

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
            println!("{:?}", combined);
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
                        "description": "查询知识库以检索关于某个主题的相关信息。",
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
        };
        // 发送 POST 请求
        let response = client
            .post(api_key)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("Authorization", format!("Bearer {}", api_key))
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
                    let response: ChatCompletionResponse =
                        serde_json::from_str(&response_text).unwrap();

                    // if Some(tool_calls) =response.get("tool_calls") let response_prompt = {
                    //      for  tool in tool_calls.into_iter() {
                    //         if tool.
                    //      }
                    // };
                    {
                        let mut history_messages = history_messages.lock().await;
                        history_messages.push(response.choices[0].message.clone());
                    };

                    return response.choices[0].message.content.to_string();
                } else {
                    eprintln!("Failed to get a successful response: {}", response.status());
                    return format!("Error: {:?}", response.status());
                }
            }
            Err(e) => {
                eprintln!("Failed to send request: {:?}", e);
                return format!("Error: {:?}", response.status());
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
                        &event.sender.nickname.unwrap(),
                        &event.borrow_text().unwrap()
                    ),
                    role: "user".to_string(),
                }];

                let res = chat_with_gpt(
                    client_clone,
                    api_key_clone,
                    api_url_clone,
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
