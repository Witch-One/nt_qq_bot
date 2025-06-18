// use kovi::PluginBuilder as plugin;
// use rusqlite::{params, Connection, Result};
// use rusqlite::types::Value;

// #[kovi::plugin]
// async fn main() {
//     let conn = Connection::open("my_database.db")?;

//     // 创建表
//     conn.execute(
//         "CREATE TABLE IF NOT EXISTS users (
//             id INTEGER PRIMARY KEY AUTOINCREMENT,
//             name TEXT NOT NULL,
//             age INTEGER NOT NULL
//         )",
//         params![],
//     )?;

//     // 插入数据
//     conn.execute(
//         "INSERT INTO users (name, age) VALUES (?1, ?2)",
//         params!["Alice", 30],
//     )?;


//     plugin::on_msg(|event| async move {
//         if event.raw_message.contains("[CQ:at,qq=3939271104]")
//                 && event.borrow_text().unwrap().to_string().starts_with("语录")
//             {
//                 let message = event
//                     .original_json
//                     .get("message")
//                     .and_then(|m| m.as_array()).unwrap();

//                 let msg_url = for msg in message.into_iter() {
//                     println!("here:{:?}", msg);
//                     if msg.get("type") == Some(&serde_json::Value::String("image".to_string())) {
//                         return msg
//                             .get("data")
//                             .and_then(|data| data.get("url"))
//                             .unwrap_or(&serde_json::Value::Null)
//                             .to_string();
//                     }
//                 };
//                 println!("here:{:?}", msg_url);
//                 event.reply(format!("{:?}", msg_url));
//             }
//             return "".to_string();
//     });
// }
