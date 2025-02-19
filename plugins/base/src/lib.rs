use base64::encode;
use kovi::PluginBuilder as plugin;
use kovi::bot::message::Message;
use std::env;
use std::fs;
#[kovi::plugin]
async fn main() {
    plugin::on_msg(|event| async move {
        if event.borrow_text() == Some("老鼠") {
            // 读取本地图片文件
            let image_path = "assets/imgs/mouse.png";
            // 获取当前工作目录
            let current_dir = env::current_dir().expect("Failed to get current directory");
            // 将相对路径与当前工作目录组合，得到绝对路径
            let absolute_path = current_dir.join(image_path);
            // 打印绝对路径
            println!("Absolute path: {:?}", absolute_path);

            // 创建包含 Base64 图片的 Message
            let msg = Message::new().add_image(&image_path);
            event.reply(msg);
        }
    });
}
