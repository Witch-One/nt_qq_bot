use kovi::PluginBuilder as plugin;
use kovi::tokio;
use kovi::tokio::sync::Mutex; // 使用 tokio 的 Mutex
use rand::Rng;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::error::Error;
use std::sync::Arc;
use std::env;

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

#[derive(Debug, Deserialize, Serialize, Clone)]
struct TaroCard {
    index: u32,
    name: &'static str,
    description: &'static str,
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
    let api_key = env::var("SILICON_FLOW_API_KEY").unwrap_or_default();
    println!("{}",api_key);
    let api_url = "https://api.siliconflow.cn/v1/chat/completions";

    let tarot_cards = vec![
        TaroCard {
            index: 0,
            name: "愚者（The Fool）",
            description: "新的开始、冒险、自由、未知",
        },
        TaroCard {
            index: 1,
            name: "魔术师（The Magician）",
            description: "创造力、掌控、意志、潜力",
        },
        TaroCard {
            index: 2,
            name: "女祭司（The High Priestess）",
            description: "直觉、神秘、智慧、潜意识",
        },
        TaroCard {
            index: 3,
            name: "皇后（The Empress）",
            description: "繁荣、母性、创造、丰盛",
        },
        TaroCard {
            index: 4,
            name: "皇帝（The Emperor）",
            description: "规则、权威、稳定、责任",
        },
        TaroCard {
            index: 5,
            name: "教皇（The Hierophant）",
            description: "传统、信仰、指导、智慧",
        },
        TaroCard {
            index: 6,
            name: "恋人（The Lovers）",
            description: "爱情、关系、选择、和谐",
        },
        TaroCard {
            index: 7,
            name: "战车（The Chariot）",
            description: "意志力、胜利、掌控、自律",
        },
        TaroCard {
            index: 8,
            name: "力量（Strength）",
            description: "内在力量、耐心、勇气、控制",
        },
        TaroCard {
            index: 9,
            name: "隐士（The Hermit）",
            description: "内省、智慧、寻找真相、孤独",
        },
        TaroCard {
            index: 10,
            name: "命运之轮（Wheel of Fortune）",
            description: "变化、命运、循环、机遇",
        },
        TaroCard {
            index: 11,
            name: "正义（Justice）",
            description: "公正、平衡、因果、真相",
        },
        TaroCard {
            index: 12,
            name: "倒吊人（The Hanged Man）",
            description: "牺牲、放下、顿悟、新视角",
        },
        TaroCard {
            index: 13,
            name: "死神（Death）",
            description: "结束、新生、转变、蜕变",
        },
        TaroCard {
            index: 14,
            name: "节制（Temperance）",
            description: "平衡、耐心、和谐、适度",
        },
        TaroCard {
            index: 15,
            name: "恶魔（The Devil）",
            description: "诱惑、束缚、沉迷、物欲",
        },
        TaroCard {
            index: 16,
            name: "塔（The Tower）",
            description: "突发变化、毁灭、觉醒、重建",
        },
        TaroCard {
            index: 17,
            name: "星星（The Star）",
            description: "希望、灵性指引、启示、治愈",
        },
        TaroCard {
            index: 18,
            name: "月亮（The Moon）",
            description: "潜意识、幻象、不安、直觉",
        },
        TaroCard {
            index: 19,
            name: "太阳（The Sun）",
            description: "快乐、成功、积极、能量",
        },
        TaroCard {
            index: 20,
            name: "审判（Judgement）",
            description: "觉醒、复苏、决定、救赎",
        },
        TaroCard {
            index: 21,
            name: "世界（The World）",
            description: "完成、成就、整合、圆满",
        },
    ];

    fn get_card(tarot_cards: &[TaroCard]) -> TaroCard {
        // 获取随机数生成器
        let mut rng = rand::thread_rng();
        // 生成一个随机索引
        let random_index = rng.gen_range(0..tarot_cards.len());
        // 通过随机索引访问 Vec 中的元素
        tarot_cards[random_index].clone()
    }

    // 创建 HTTP 客户端
    let client = Client::new();

    plugin::on_msg(move |event| {
        let client_clone = client.clone();
        let api_url_clone = api_url.to_string();
        let api_key_clone = api_key.to_string();
        let tarot_cards_clone = tarot_cards.clone();
        println!(
            "{:?}",
            event
                .borrow_text()
                .unwrap()
                .to_string()
                .starts_with("运势 ")
        );

        async move {
            if event.borrow_text().unwrap().to_string().starts_with("运势")
                && !event.raw_message.contains("[CQ:at,qq=3939271104]")
            {
                // event.raw_message.contains("[CQ:at,qq=3939271104]") ||

                let mut history_messages: Vec<Message> = vec![Message {
                    content: "你是一个专业的塔罗牌占卜师,我会将客人抽到的三张牌的名字和正反位发给你,请你根据客人的问题帮他解答。
                    注意关于解牌不能过于美化,不能曲解牌面本来的意义。
                    解答需要简洁明了,不要犹豫不决。
                    你的客户都是不懂塔罗牌的客户,只想知道关于他的问题的答案或者是他最近的运势情况,不要用神秘无意义的话术回答,解释一下牌的意义以及组合牌面回答问题即可。".to_string(),
                    role: "system".to_string(),
                }];

                for i in 0..3 {
                    let card = get_card(&tarot_cards_clone);
                    let mut rng = rand::thread_rng();
                    let is_upright = rng.gen_bool(0.5);
                    let position = if is_upright { "正位" } else { "反位" };
                    history_messages.push(Message {
                        role: "user".to_string(),
                        content: format!("牌名: {}, {}", card.name, position),
                    });
                }

                let question =
                    match remove_prefix_if_starts_with(event.borrow_text().unwrap(), "运势 ") {
                        Some(question) => question,
                        None => "用户没有问题,请按照牌面解答一下最近的运势以及可能会碰到的事"
                            .to_string(),
                    };

                history_messages.push(Message {
                    role: "user".to_string(),
                    content: question,
                });

                let request_body = json!({
                    "messages":history_messages,
                    "model": "Pro/deepseek-ai/DeepSeek-R1",
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
                });

                println!("{}", request_body);

                // 发送 POST 请求
                let response = client_clone
                    .post(api_url_clone)
                    .header("Content-Type", "application/json")
                    .header("Accept", "application/json")
                    .header("Authorization", format!("Bearer {}", api_key_clone))
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
                            event.reply_and_quote(&response.choices[0].message.content);
                        } else {
                            eprintln!("Failed to get a successful response: {}", response.status());
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to send request: {:?}", e);
                    }
                }
            }
        }
    });
}
